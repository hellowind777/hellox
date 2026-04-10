mod jaccard;
mod tfidf;
mod tokenize;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{anyhow, Result};

use crate::{
    list_archived_memories, list_memories, load_archived_memory, load_memory, relative_age_text,
    MemoryEntry, MemoryScope,
};

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClusterOptions {
    pub archived: bool,
    pub limit: usize,
    pub min_jaccard: f32,
    pub max_tokens: usize,
    pub semantic: bool,
}

impl Default for MemoryClusterOptions {
    fn default() -> Self {
        Self {
            archived: false,
            limit: 200,
            min_jaccard: 0.18,
            max_tokens: 48,
            semantic: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryClusterMember {
    pub memory_id: String,
    pub scope: MemoryScope,
    pub updated_at: u64,
    pub age: String,
    pub preview: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCluster {
    pub cluster_id: usize,
    pub seed_memory_id: String,
    pub seed_preview: String,
    pub members: Vec<MemoryClusterMember>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemoryClusterReport {
    pub archived: bool,
    pub semantic: bool,
    pub limit: usize,
    pub min_jaccard: f32,
    pub max_tokens: usize,
    pub clusters: Vec<MemoryCluster>,
    pub entries: usize,
}

pub fn cluster_memories(
    root: &Path,
    options: &MemoryClusterOptions,
) -> Result<MemoryClusterReport> {
    validate_options(options)?;

    let mut entries = if options.archived {
        list_archived_memories(root)?
    } else {
        list_memories(root)?
    };
    entries.truncate(options.limit);

    let inputs = build_inputs(root, options.archived, &entries, options.max_tokens)?;

    let clusters = if options.semantic {
        cluster_semantic(&inputs, options.min_jaccard)
    } else {
        cluster_token_overlap(&inputs, options.min_jaccard)
    };

    Ok(MemoryClusterReport {
        archived: options.archived,
        semantic: options.semantic,
        limit: options.limit,
        min_jaccard: options.min_jaccard,
        max_tokens: options.max_tokens,
        clusters,
        entries: inputs.len(),
    })
}

pub fn format_memory_cluster_report(report: &MemoryClusterReport) -> String {
    if report.entries == 0 {
        return "No memory files found.".to_string();
    }

    let mode = if report.semantic {
        "tfidf_cosine"
    } else {
        "token_overlap"
    };

    let mut lines = vec![format!(
        "Clustered {} memory file(s) into {} cluster(s) (mode: {}, archived: {}, limit: {}, min_similarity: {}, max_tokens: {}).",
        report.entries,
        report.clusters.len(),
        mode,
        report.archived,
        report.limit,
        report.min_jaccard,
        report.max_tokens
    )];

    if report.clusters.is_empty() {
        return lines.join("\n");
    }

    lines.push("cluster_id\tseed_memory_id\tseed_preview\tmemory_id\tscope\tage\tupdated_at\tpreview\tpath".to_string());
    for cluster in &report.clusters {
        for member in &cluster.members {
            lines.push(format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                cluster.cluster_id,
                cluster.seed_memory_id,
                escape_tsv(&cluster.seed_preview),
                member.memory_id,
                member.scope.as_str(),
                member.age,
                member.updated_at,
                escape_tsv(&member.preview),
                member.path
            ));
        }
    }
    lines.join("\n")
}

fn validate_options(options: &MemoryClusterOptions) -> Result<()> {
    if options.limit == 0 {
        return Err(anyhow!("memory clusters limit must be at least 1"));
    }
    if !(0.0..=1.0).contains(&options.min_jaccard) {
        return Err(anyhow!(
            "memory clusters min_jaccard must be between 0.0 and 1.0"
        ));
    }
    if options.max_tokens == 0 {
        return Err(anyhow!("memory clusters max_tokens must be at least 1"));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct ClusterInput {
    member: MemoryClusterMember,
    token_set: HashSet<String>,
    token_counts: HashMap<String, u32>,
}

fn build_inputs(
    root: &Path,
    archived: bool,
    entries: &[MemoryEntry],
    max_tokens: usize,
) -> Result<Vec<ClusterInput>> {
    let mut inputs = Vec::with_capacity(entries.len());
    for entry in entries {
        let markdown = if archived {
            load_archived_memory(root, &entry.memory_id)?
        } else {
            load_memory(root, &entry.memory_id)?
        };

        let preview = tokenize::summary_first_line(&markdown);
        let token_counts = tokenize::token_counts(&markdown, &preview, max_tokens);
        let token_set = token_counts.keys().cloned().collect::<HashSet<_>>();

        inputs.push(ClusterInput {
            member: MemoryClusterMember {
                memory_id: entry.memory_id.clone(),
                scope: entry.scope,
                updated_at: entry.updated_at,
                age: relative_age_text(entry.updated_at),
                preview,
                path: entry.path.clone(),
            },
            token_set,
            token_counts,
        });
    }
    Ok(inputs)
}

fn cluster_token_overlap(inputs: &[ClusterInput], min_similarity: f32) -> Vec<MemoryCluster> {
    let mut clusters: Vec<TokenOverlapClusterBuilder> = Vec::new();

    for (input_index, input) in inputs.iter().enumerate() {
        let mut best_index = None;
        let mut best_score = 0.0_f32;

        for (cluster_index, cluster) in clusters.iter().enumerate() {
            let seed_tokens = &inputs[cluster.seed_input_index].token_set;
            let score = jaccard::similarity(seed_tokens, &input.token_set);
            if score > best_score {
                best_score = score;
                best_index = Some(cluster_index);
            }
        }

        let member = input.member.clone();
        match best_index {
            Some(index) if best_score >= min_similarity => clusters[index].members.push(member),
            _ => clusters.push(TokenOverlapClusterBuilder::new(
                input_index,
                member,
                input.member.preview.clone(),
            )),
        }
    }

    clusters
        .into_iter()
        .enumerate()
        .map(|(index, cluster)| MemoryCluster {
            cluster_id: index + 1,
            seed_memory_id: cluster.seed_memory_id,
            seed_preview: cluster.seed_preview,
            members: cluster.members,
        })
        .collect()
}

fn cluster_semantic(inputs: &[ClusterInput], min_similarity: f32) -> Vec<MemoryCluster> {
    let docs = inputs
        .iter()
        .map(|input| &input.token_counts)
        .collect::<Vec<_>>();
    let vectors = tfidf::build_vectors(&docs);

    let mut clusters: Vec<SemanticClusterBuilder> = Vec::new();

    for (input_index, input) in inputs.iter().enumerate() {
        let candidate = &vectors[input_index];

        let mut best_index = None;
        let mut best_score = 0.0_f32;
        for (cluster_index, cluster) in clusters.iter().enumerate() {
            let seed = &vectors[cluster.seed_input_index];
            let score = tfidf::cosine_similarity(seed, candidate);
            if score > best_score {
                best_score = score;
                best_index = Some(cluster_index);
            }
        }

        let member = input.member.clone();
        match best_index {
            Some(index) if best_score >= min_similarity => clusters[index].members.push(member),
            _ => clusters.push(SemanticClusterBuilder::new(
                input_index,
                member,
                input.member.preview.clone(),
            )),
        }
    }

    clusters
        .into_iter()
        .enumerate()
        .map(|(index, cluster)| MemoryCluster {
            cluster_id: index + 1,
            seed_memory_id: cluster.seed_memory_id,
            seed_preview: cluster.seed_preview,
            members: cluster.members,
        })
        .collect()
}

struct TokenOverlapClusterBuilder {
    seed_input_index: usize,
    seed_memory_id: String,
    seed_preview: String,
    members: Vec<MemoryClusterMember>,
}

impl TokenOverlapClusterBuilder {
    fn new(seed_input_index: usize, seed: MemoryClusterMember, seed_preview: String) -> Self {
        Self {
            seed_input_index,
            seed_memory_id: seed.memory_id.clone(),
            seed_preview,
            members: vec![seed],
        }
    }
}

struct SemanticClusterBuilder {
    seed_input_index: usize,
    seed_memory_id: String,
    seed_preview: String,
    members: Vec<MemoryClusterMember>,
}

impl SemanticClusterBuilder {
    fn new(seed_input_index: usize, seed: MemoryClusterMember, seed_preview: String) -> Self {
        Self {
            seed_input_index,
            seed_memory_id: seed.memory_id.clone(),
            seed_preview,
            members: vec![seed],
        }
    }
}

fn escape_tsv(value: &str) -> String {
    value
        .replace('\t', " ")
        .replace('\n', " ")
        .replace('\r', " ")
}

#[cfg(test)]
mod tests;

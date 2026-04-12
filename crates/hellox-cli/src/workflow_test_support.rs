use std::sync::{Arc, LazyLock};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static WORKFLOW_TEST_SEMAPHORE: LazyLock<Arc<Semaphore>> =
    LazyLock::new(|| Arc::new(Semaphore::new(1)));

pub(crate) async fn acquire_workflow_test_guard() -> OwnedSemaphorePermit {
    WORKFLOW_TEST_SEMAPHORE
        .clone()
        .acquire_owned()
        .await
        .expect("acquire workflow test guard")
}

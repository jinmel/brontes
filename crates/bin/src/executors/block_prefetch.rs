use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use alloy_provider::{Provider, RootProvider};
use alloy_pubsub::{PubSubFrontend, Subscription};
use alloy_rpc_types::Block;
use brontes_database::libmdbx::LibmdbxInit;
use brontes_types::executor::BrontesTaskExecutor;
use futures::{ready, stream::FuturesUnordered, Future, FutureExt, StreamExt};
use tokio::sync::broadcast::error::RecvError;

struct BlockSubscriber<DB: LibmdbxInit> {
    executor:           BrontesTaskExecutor,
    libmdbx:            &'static DB,
    provider:           Arc<RootProvider<PubSubFrontend>>,
    processing_futures: FuturesUnordered<JoinHandle<()>>,
}

impl<DB: LibmdbxInit> BlockSubscriber<DB> {
    pub fn new(
        executor: BrontesTaskExecutor,
        libmdbx: &'static DB,
        provider: Arc<RootProvider<PubSubFrontend>>,
    ) -> Self {
        Self { executor, libmdbx, provider, processing_futures: FuturesUnordered::new() }
    }

    pub async fn run(&mut self) {}

    pub async fn prefetch_blocks(&mut self) {
        // fires a future to prefetch blocks, queueing them into processing_futures
        let prefetch_future = self.provider.subscribe_blocks().boxed();
        let executor = self.executor.clone();
        let provider = self.provider.clone();
        self.processing_futures
            .push(self.executor.spawn_critical("block_fetcher", async move {
                let mut sub = provider.subscribe_blocks().await;
                if let Ok(sub) = sub {
                    while let res = sub.recv().await {
                        match res {
                            Ok(block) => if let Some(number) = block.header.number {                                                                
                            },
                            Err(RecvError::Closed) => {
                                tracing::warn!("block_fetcher: subscription closed, resubscribing");
                                sub = sub.resubscribe();
                                break;
                            }
                            Err(RecvError::Lagged(b)) => {
                                tracing::warn!("block_fetcher: lagged by {} blocks", b);
                                continue;
                            }
                        }
                    }
                }
                ()
            }));
    }

    pub async fn get_metadata_and_insert(&mut self) {}
}

impl<DB: LibmdbxInit> Future for BlockSubscriber<DB> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        while match self.processing_futures.poll_next_unpin(cx) {
            Poll::Ready(Some(_)) => true,
            Poll::Ready(None) => return Poll::Ready(()),
            Poll::Pending => return Poll::Pending,
        } {}

        Poll::Pending
    }
}

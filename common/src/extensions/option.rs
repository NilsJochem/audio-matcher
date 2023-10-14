use std::future::Future;

/// extentions for Options<Future<_>>
#[async_trait::async_trait]
pub trait FutureExt {
    /// The type of value produced on completion.
    type Type;
    /// returns the hold value or computes `f` and inserts it
    async fn get_or_try_insert_future<F: Future<Output = Option<Self::Type>> + Send>(
        &mut self,
        f: F,
    ) -> Option<&mut Self::Type>;
    #[allow(missing_docs)]
    async fn get_or_insert_future<F: Future<Output = Self::Type> + Send>(
        &mut self,
        f: F,
    ) -> &mut Self::Type {
        self.get_or_try_insert_future(async { Some(f.await) })
            .await
            .unwrap()
    }
    #[allow(missing_docs)]
    async fn insert_future_if_none<F: Future<Output = Self::Type> + Send>(&mut self, f: F) {
        let _ = self.get_or_insert_future(f).await;
    }
    #[allow(missing_docs)]
    async fn try_inser_futuret_if_none<F: Future<Output = Option<Self::Type>> + Send>(
        &mut self,
        f: F,
    ) {
        let _ = self.get_or_try_insert_future(f).await;
    }
}
#[async_trait::async_trait]
impl<T: Send> FutureExt for Option<T> {
    type Type = T;
    async fn get_or_try_insert_future<F: Future<Output = Self> + Send>(
        &mut self,
        f: F,
    ) -> Option<&mut Self::Type> {
        if self.is_none() {
            f.await.map(|t| self.insert(t))
        } else {
            self.as_mut()
        }
    }
}

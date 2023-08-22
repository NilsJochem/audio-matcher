use std::future::Future;

#[async_trait::async_trait]
pub trait FutureExt {
    type Type;
    async fn get_or_try_insert_future<F: Future<Output = Option<Self::Type>> + Send>(
        &mut self,
        f: F,
    ) -> Option<&mut Self::Type>;
    async fn get_or_insert_future<F: Future<Output = Self::Type> + Send>(
        &mut self,
        f: F,
    ) -> &mut Self::Type {
        self.get_or_try_insert_future(async { Some(f.await) })
            .await
            .unwrap()
    }
    async fn insert_future_if_none<F: Future<Output = Self::Type> + Send>(&mut self, f: F) {
        let _ = self.get_or_insert_future(f).await;
    }
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

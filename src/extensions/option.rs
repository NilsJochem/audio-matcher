use std::future::Future;

pub trait Ext {
    type Type;
    fn get_or_try_insert<F: FnOnce() -> Option<Self::Type>>(
        &mut self,
        f: F,
    ) -> Option<&mut Self::Type>;
    fn get_or_insert<F: FnOnce() -> Self::Type>(&mut self, f: F) -> &mut Self::Type {
        self.get_or_try_insert(|| Some(f())).unwrap()
    }
    fn insert_if_none<F: FnOnce() -> Self::Type>(&mut self, f: F) {
        let _ = self.get_or_insert(f);
    }
    fn try_insert_if_none<F: FnOnce() -> Option<Self::Type>>(&mut self, f: F) {
        let _ = self.get_or_try_insert(f);
    }
}
impl<T> Ext for Option<T> {
    type Type = T;
    fn get_or_try_insert<F: FnOnce() -> Self>(&mut self, f: F) -> Option<&mut Self::Type> {
        if self.is_none() {
            f().map(|t| self.insert(t))
        } else {
            self.as_mut()
        }
    }
}
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

use async_trait::async_trait;

#[async_trait]
pub trait Backend {
    fn get_lock_key(&self) -> &str;
    async fn get_lock(&self) -> Option<String>;
}

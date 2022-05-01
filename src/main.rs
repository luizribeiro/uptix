use dkregistry::v2::Client;

async fn get_digest<'a>(
    registry: &'a str,
    image: &'a str,
    tag: &'a str,
) -> Option<String> {
    let client = Client::configure().registry(registry).build().unwrap();
    let login_scope = format!("repository:{}:pull", image);
    let dclient = client.authenticate(&[&login_scope]).await.unwrap();
    return dclient.get_manifestref(image, tag).await.unwrap();
}

#[tokio::main]
async fn main() {
    let digest = get_digest("ghcr.io", "home-assistant/home-assistant", "stable").await.unwrap();
    println!("digest: {}", digest);
}

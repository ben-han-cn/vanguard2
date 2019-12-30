use std::fs;
use tokio::runtime::Runtime;
use tonic;
use vanguard2::controller::{AddZoneRequest, DynamicUpdateInterfaceClient};

fn main() {
    let mut rt = Runtime::new().unwrap();
    rt.block_on(async {
        let mut client = DynamicUpdateInterfaceClient::connect("http://127.0.0.1:5556")
            .await
            .unwrap();
        let request = tonic::Request::new(AddZoneRequest {
            zone: "example.org".to_string(),
            zone_content: fs::read_to_string(
                "/home/vagrant/workspace/code/rust/vanguard2/testdata/example.org.zone",
            )
            .unwrap(),
        });
        let response = client.add_zone(request).await.unwrap();
        println!("RESPONSE={:?}", response);
    });
}

use openrpc_derive::openrpc;

#[openrpc]
pub trait DebugApi {
    /// panic function
    #[rpc(name = "debug.panic")]
    fn panic(&self, me: String) -> Result<String, &str>;

    ///Only can used under dev net.
    #[rpc(name = "debug.sleep")]
    fn sleep(&self, time: u64) -> Result<String, &str>;
}

fn main() {
    let schema = openrpc_schema_DebugApi::gen_schema();
    let j = serde_json::to_string_pretty(&schema).unwrap();
    println!("{}", j);
}

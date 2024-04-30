# Semantic Conventions for Rust

# Usage

```rust
fn main() {
    // Display the KeyValue of the attribute CLIENT_ADDRESS initialized with the value "145.34.23.56"
    println!("{:?}", semconv::client::CLIENT_ADDRESS.value("145.34.23.56".into()));
    // Display the key of the attribute CLIENT_ADDRESS
    println!("{:?}", semconv::client::CLIENT_ADDRESS.key());
    
    // Display the KeyValue of the attribute CLIENT_PORT initialized with the value 8080
    println!("{:?}", semconv::client::CLIENT_PORT.value(8080));
    // Display the key of the attribute CLIENT_PORT
    println!("{:?}", semconv::client::CLIENT_PORT.key());

    // Display the string representation of the enum variant HttpRequestMethod::Connect
    println!("{}", semconv::http::HttpRequestMethod::Connect);
}
```
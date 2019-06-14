use docopt::Docopt;

fn main() {
    let usage = format!(
        "Usage: partial-eth-key-cracker --key=<key> --fullnode=<fullnode>
Options:
    --key                       The partial private key to crack
    --fullnode                  The fullnode used to check the balance
About:
    Version {}
    git hash {}",
        env!("CARGO_PKG_VERSION"),
        env!("GIT_HASH")
    );
    let args = Docopt::new(usage);

    let system = actix::System::new("key-cracker");

    println!("Hello, world!");
    system.run().unwrap();
}

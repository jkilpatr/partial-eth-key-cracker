use actix::{Actor, Addr, Arbiter, AsyncContext, Context, Handler, Message};
use clarity::Address;
use clarity::PrivateKey;
use docopt::Docopt;
use failure::bail;
use failure::Error;
use futures::future;
use futures::future::Future;
use hex::decode;
use num_cpus;
use rand::distributions::{Distribution, Uniform};
use serde::Deserialize;
use std::str::FromStr;
use std::thread;
use std::time::Duration;
use std::time::Instant;
use sysinfo::SystemExt;
use tokio::timer::Delay;
use web30::client::Web3;

struct CheckKeys {
    count: u128,
    max: u128,
    history: (Instant, u128),
}

impl Actor for CheckKeys {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        println!("Balance checking event loop started");
        ctx.set_mailbox_capacity(1000);
        ctx.run_interval(Duration::from_secs(5), |_act, ctx| {
            let addr: Addr<Self> = ctx.address();
            addr.do_send(Tick);
        });
    }
}

pub struct CheckBalance {
    private_key: PrivateKey,
    address: Address,
    full_node: String,
}

impl Message for CheckBalance {
    type Result = Result<(), Error>;
}

impl Handler<CheckBalance> for CheckKeys {
    type Result = Result<(), Error>;
    fn handle(&mut self, msg: CheckBalance, ctx: &mut Context<Self>) -> Self::Result {
        let full_node = msg.full_node;
        let address = msg.address;
        let test_key = msg.private_key;
        let web3 = Web3::new(&full_node, Duration::from_secs(1));
        // done to keep the borrow checker happy
        let local_address_a = ctx.address().clone();
        let local_address_b = ctx.address().clone();

        let fut = web3
            .eth_get_balance(address)
            .and_then(move |balance| {
                if balance != 0u32.into() {
                    println!("Found a key! {} {}", test_key.to_string(), address);
                    panic!("look at the key {} {}!", test_key.to_string(), address);
                } else {
                    //println!("{} contains no funds", address);
                    local_address_a.do_send(Count);
                }
                Ok(()) as Result<(), Error>
            })
            .then(move |val| {
                if let Err(_e) = val {
                    //println!("Got error {:?} getting balance", e);
                    let step = Uniform::new(0, 10000);
                    let mut rng = rand::thread_rng();
                    let choice = step.sample(&mut rng);
                    let when = Instant::now() + Duration::from_millis(choice);

                    Box::new(
                        Delay::new(when)
                            .and_then(move |_| {
                                local_address_b.do_send(CheckBalance {
                                    private_key: test_key,
                                    address: address,
                                    full_node: full_node.clone(),
                                });
                                Ok(())
                            })
                            .then(|_| Ok(())),
                    )
                } else {
                    Box::new(future::ok(())) as Box<Future<Item = (), Error = ()>>
                }
            });

        Arbiter::spawn(fut);
        Ok(())
    }
}

pub struct Tick;

impl Message for Tick {
    type Result = Result<(), Error>;
}

impl Handler<Tick> for CheckKeys {
    type Result = Result<(), Error>;
    fn handle(&mut self, _: Tick, _ctx: &mut Context<Self>) -> Self::Result {
        let (last_sample_time, last_sample_number) = self.history;
        let time_since_last_sample = (Instant::now() - last_sample_time).as_secs();
        let progress = self.count as f32 / self.max as f32;
        let avg = (self.count - last_sample_number) / time_since_last_sample as u128;
        self.history = (Instant::now(), self.count);

        println!(
            "Checking keys: {} keys / second \nProgress: {:.2}%",
            avg, progress
        );
        Ok(())
    }
}

pub struct Count;

impl Message for Count {
    type Result = Result<(), Error>;
}

impl Handler<Count> for CheckKeys {
    type Result = Result<(), Error>;
    fn handle(&mut self, _: Count, _ctx: &mut Context<Self>) -> Self::Result {
        self.count += 1;
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct Args {
    flag_key: String,
    flag_fullnode: String,
    flag_known_public_key: String,
    flag_start_index: u8,
    flag_end_index: u8,
}

fn spawn_check_futures(
    known_address: Option<Address>,
    partial_key: Vec<u8>,
    full_node: String,
    num_scratch_bytes: u8,
    start: u8,
    end: u8,
    cores: u16,
    addr: Addr<CheckKeys>,
) {
    // spawn a thread so that the caller can progress and begin to service
    // CheckBalance messages as we perform the generation
    thread::spawn(move || {
        let mut test_keys = Vec::new();
        let mut scratch_bits = zeros(num_scratch_bytes);

        let test_key_bytes = overwirite_scratch_bits(&partial_key, &scratch_bits, start, end);
        // there's no do_while loop in rust so we add the all zeros key before entering
        // the incrementing loop
        test_keys.push(test_key_bytes);

        while increment_vec_uint(&mut scratch_bits).is_ok() {
            let test_key_bytes = overwirite_scratch_bits(&partial_key, &scratch_bits, start, end);
            test_keys.push(test_key_bytes);
        }

        let total_keys = test_keys.len();
        let keys_per_worker = total_keys / cores as usize;
        let mut keys_assigned = 0;
        let mut children = Vec::new();

        // split up key generation into a threadpool, mostly not needed since resolution is
        // the bottleneck
        while keys_assigned < total_keys {
            let start = keys_assigned;
            keys_assigned += keys_per_worker;
            let end = if keys_assigned + keys_per_worker > total_keys {
                total_keys - 1
            } else {
                keys_assigned
            };
            let thread_keys: Vec<Vec<u8>> = test_keys[start..end].iter().cloned().collect();
            let local_full_node = full_node.clone();
            let local_addr = addr.clone();

            children.push(thread::spawn(move || {
                let mut counter = 0;
                for key in thread_keys {
                    counter += 1;
                    // this is somewhat expensive, run it less freqently
                    if counter % 10000 == 0 {
                        wait_for_memory_pressure();
                    }
                    let test_key = PrivateKey::from_slice(&key).unwrap();
                    //println!("Testing key {}", test_key.to_string());
                    let public_key = test_key.to_public_key().unwrap();

                    match known_address {
                        // backpressure, if the mailbox is full sleep and wait for it to not be
                        None => {
                            while local_addr
                                .send(CheckBalance {
                                    private_key: test_key,
                                    address: public_key,
                                    full_node: local_full_node.clone(),
                                })
                                .wait()
                                .is_err()
                            {
                                println!("backpressure!");
                                thread::sleep(Duration::from_secs(5));
                            }
                        }
                        Some(key) => {
                            local_addr.do_send(Count);
                            if test_key.to_public_key().unwrap() == key {
                                println!("Found a key! {} {}", test_key.to_string(), key);
                                panic!("look at the key {} {}!", test_key.to_string(), key);
                            }
                        }
                    }
                }
            }));
        }

        for child in children {
            child.join().expect("Error in key generation worker!");
        }

        println!("{} keys generated", test_keys.len());
    });
}

/// Increment a unit presetned as a arbitrary length vec
fn increment_vec_uint(num: &mut Vec<u8>) -> Result<(), Error> {
    for val in num.iter_mut() {
        if *val < 255 {
            *val += 1;
            return Ok(());
        } else if *val == 255 {
            *val = 0;
        }
    }
    bail!("We have reached the end of our search space!");
}

fn zeros(size: u8) -> Vec<u8> {
    vec![0; size as usize]
}

/// Takes the key and scratch bits and uses them to overwrite the value in the key
fn overwirite_scratch_bits(key: &Vec<u8>, scratch: &Vec<u8>, start: u8, end: u8) -> Vec<u8> {
    let mut count = 0;
    let mut key = key.clone();
    for index in start..end {
        key[index as usize] = scratch[count];
        count += 1;
    }
    key
}

/// Checking keys is much slower than generating them, in order to not to run out of memory
/// we have to stop feeding keys into Actix and wait for them to drain out and be checked
/// this function checks system memory and if usage is over 75% it halts the worker threads
fn wait_for_memory_pressure() {
    let system = sysinfo::System::new();
    let total_mem = system.get_total_memory() as f32;
    let used_mem = system.get_used_memory() as f32;
    //println!("{}", used_mem / total_mem);
    while used_mem / total_mem > 0.8 {
        println!("Halting processing while keys are checked");
        thread::sleep(Duration::from_secs(5));
    }
}

fn main() {
    let usage = format!(
        "Usage: partial-eth-key-cracker --key=<key>  --start-index=<start_index> --end-index=<end_index> [--known-public-key=<known_public_key> | --fullnode=<fullnode>]
Options:
    --key=<key>                           The partial private key to crack
    --fullnode=<fullnode>                 The fullnode used to check the balance if no public key is known
    --start-index=<start_index>           The starting location of the partial key
    --end-index=<end_index>               The ending location of the partial key
    --known-public-key=<known_public_key> A known public key to compare against, removes the need for a full node and is much faster
About:
    Written By: Justin Kilpatrick (justin@altheamesh.com)
    Version {}",
        env!("CARGO_PKG_VERSION"),
    );
    let args: Args = Docopt::new(usage)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());
    let partial_key = decode(args.flag_key).expect("Key formatting problem!");
    assert!(args.flag_start_index < args.flag_end_index);

    let mut remote_check = false;

    if args.flag_fullnode.len() != 0 {
        remote_check = true;
    }

    let full_node = args.flag_fullnode;
    let public_key = args.flag_known_public_key;

    let start = args.flag_start_index / 2;
    let end = args.flag_end_index / 2;

    let num_scratch_bytes = end - start;
    let total_keys = 2u128.pow((num_scratch_bytes * 8) as u32);

    println!(
        "Working to generate {} keys and check them for funds",
        total_keys
    );

    if partial_key.len() < 32 {
        println!(
            "Valid keys should be padded to full length using zeros! your key is {} bytes long",
            partial_key.len()
        );
        panic!("Valid keys should be padded to full length using zeros!");
    }

    let addr = CheckKeys {
        count: 0,
        max: total_keys,
        history: (Instant::now(), 0),
    }
    .start();

    if remote_check {
        spawn_check_futures(
            None,
            partial_key,
            full_node,
            num_scratch_bytes,
            start,
            end,
            1,
            addr,
        );
    } else {
        let key = Address::from_str(&public_key).expect("Invalid public key!");
        spawn_check_futures(
            Some(key),
            partial_key,
            full_node,
            num_scratch_bytes,
            start,
            end,
            num_cpus::get() as u16,
            addr,
        );
    }

    let system = actix::System::new("key-cracker");
    system.run();
}

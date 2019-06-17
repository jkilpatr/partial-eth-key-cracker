# Partial Ethereum Key Finder

Hello user, through some terrible misfortune you are missing a small part of an Ethereum private key. 

Perhaps your seed phrase got smudged, or maybe you just copied a key without quite selecting the whole thing. Either way you have a problem, your money isn't quite lost but neither is it accessible.

In order to retrieve it you will need to check tens of thousands or millions of keys to find the missing digits.

You can generate millions of keys in the matter of a few minutes. The problem is checking which of those keys is correct in an effcient manner. There are two subsets to this problem. Known address and unknown address. 

### Unknown Address

Without knowing the address you need access to the blockchain and the very slow migration of data across the internet or from a hard disk.

Doing this this easy way, generating a key then checking that key syncronously in a single thread. Would take about 100ms for each key. Meaning a private key missing 5 digits would require 20 days of guessing and checking.  

Using high performance async Rust tools from the [Althea](https://althea.org) project this tool is about 300 times faster, checking about 3k keys a second against a remote full node using only a single core. Cracking my key in a mere 2.5 hours. 

All of Althea's tooling is designed to run on OpenWRT routers, with as little as 128mb of ram. It's the fastest Ethereum light client by no small margin. 

I didn't even bother to run mulitple parallel event loops. Which would provide another speedup proportional to the number of cores on the system. 

### Known Address

Known public key is a much easier problem from a performance standpoint. You simply spawn many threads, hand them the address and they can compare the values entierly locally.

This implementation can check about 10k keys a second per core and found my key in less than 10 minutes.

Since the tooling I'm using is designed as a light client the network operations are far more carefully optimizied. The actual cryptography code for Althea [Clarity](https://github.com/althea-mesh/clarity) is totally void of processor specific or even endian specific optimizations to maintain complete code portability to MIPS, ARM, RISC-V, and other devices. 

I have little doubt that there's at least a 10x speedup lying around for vectorization. 

### Feasibility

I estimate it's feasible to crack up to ~45ish bits of missing key. Or about two missing words of a 12 word phrase. Beyond that you go from needing a single machine to needing a super computer in only a few bits. That's exponential growth for ya.

## How to retrieve a partial key

You need to have your key in hex format in order to use this tool. If you have a brain wallet seed where you are missing a word you will need to find the word list from which it was generated and convert to hex manually.

I'm very open to pull requests that would do this automatically.

Another cool feature that this project does not have yet is the ability to handle multiple areas of corruption. Right now it can only check one contiguous area of lost bits at a time. 

After that you'll need to either download the latest release from the github releases page. Or install [Rust](https://rustup.rs/) and build partial-eth-key-Finder yourself. 

Once you have a binary just run the command 

```
Usage: partial-eth-key-cracker --key=<key>  --start-index=<start_index> --end-index=<end_index> [--known-public-key=<known_public_key> | --fullnode=<fullnode>]
Options:
    --key=<key>                           The partial private key to crack
    --fullnode=<fullnode>                 The fullnode used to check the balance if no public key is known
    --start-index=<start_index>           The starting location of the partial key
    --end-index=<end_index>               The ending location of the partial key
    --known-public-key=<known_public_key> A known public key to compare against, removes the need for a full node and is much faster
About:
    Written By: Justin Kilpatrick (justin@altheamesh.com)
    Version 0.1.0

```

For example. You'll notice that I'm missing the last 6 characters of this key. I simply replace them with zeros then pass the starting and ending index. As another example if we where missing the first 6 characters instead we would pass `--start-index=0 --end-index=5`. 

```
partial-eth-key-finder --key="ca624d654b67200f88ac278735a3cff286135253d05092f1e3ff60473f000000" --fullnode="https://mainnet.infura.io/v3/691ee6c5c04a4834a41fc28288f252e4" --start-index=58 --end-index=64
```

or

```
partial-eth-key-finder --key="ca624d654b67200f88ac278735a3cff286135253d05092f1e3ff60473f000000" --known-public-key="0xee1aB0e552C9dfd1698dB35CC7c5e5Ff5FD4362e" --start-index=58 --end-index=64
```

You can use your own full node, but don't bother unless you have a very fast disk drive. There's no risk of the full node stealing the private key you are trying to crack becuase only public key hashes are sent off of your machine. So the only possible benefit is performance.

#[test]
fn generate_pubkey_benchmark() {
    use crate::spl;
    use solana_sdk::signature::Keypair;
    use solana_sdk::signature::Signer;

    let payer = Keypair::new();

    let mut time_array = [0u128; 1000];
    for i in 0..1000 {
        let instant = std::time::Instant::now();
        spl::generate_pubkey(
            &payer.pubkey(), &spl_token::id(), ""
        );
        time_array[i] = instant.elapsed().as_micros();
        println!("Time elapsed in generate_pubkey() is: {:?}", instant.elapsed());
    }
    let mut sum = 0;
    for i in 0..1000 {
        sum += time_array[i];
    }
    println!("Average time elapsed in generate_pubkey() is: {:?} ms", sum/1000);
}
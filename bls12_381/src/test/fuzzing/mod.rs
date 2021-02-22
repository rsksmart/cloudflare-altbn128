extern crate hex;

#[test]
fn run_on_input() {
    let filename = "slow-unit-f3a20e0db984a4a6759a80b01af14b7b6ae30367";
    use std::time::Instant;
    use std::io::Read;
    use std::fs::File;
    let mut file = File::open(&format!("fuzz/artifacts/fuzz_target_api/{}", filename)).expect("must open");
    let mut input_data = vec![];
    file.read_to_end(&mut input_data).expect("must read");
    assert!(input_data.len() != 0);
    let now = Instant::now();
    let result = crate::public_interface::API::run(&input_data[..]);
    let elapsed = now.elapsed().as_micros();
    let gas_estimate = crate::gas_meter::GasMeter::meter(&input_data[..]);
    if result.is_err() {
        println!("Api call failed in {} micros with error {:?}", elapsed, result.err());
        if gas_estimate.is_ok() {
            println!("Gas estimate was {}", gas_estimate.unwrap());
        } else {
            println!("Gas estimate failed");
        }
    } else {
        println!("Api call was ok in {} micros", elapsed);
        if gas_estimate.is_ok() {
            println!("Gas estimate was {}", gas_estimate.unwrap());
        } else {
            println!("Gas estimate failed");
        }
        println!("Result = {}", hex::encode(&result.unwrap()));
    }
}

#[test]
fn run_on_hongg_input() {
    let filename = "SIGABRT.PC.7ffff6e56e97.STACK.1b8e87e9e5.CODE.-6.ADDR.(nil).INSTR.mov____0x108(%rsp),%rcx.fuzz";
    use std::time::Instant;
    use std::io::Read;
    use std::fs::File;
    let mut file = File::open(&format!("../eip1962_fuzzing/honggfuzz/hfuzz_workspace/fuzz_target_compare/{}", filename)).expect("must open");
    let mut input_data = vec![];
    file.read_to_end(&mut input_data).expect("must read");
    assert!(input_data.len() != 0);
    println!("Input = {}", hex::encode(&input_data));
    let now = Instant::now();
    let result = crate::public_interface::API::run(&input_data[..]);
    let elapsed = now.elapsed().as_micros();
    let gas_estimate = crate::gas_meter::GasMeter::meter(&input_data[..]);
    if result.is_err() {
        println!("Api call failed in {} micros with error {:?}", elapsed, result.err());
        if gas_estimate.is_ok() {
            println!("Gas estimate was {}", gas_estimate.unwrap());
        } else {
            println!("Gas estimate failed with error {}", gas_estimate.err().unwrap());
        }
    } else {
        println!("Api call was ok in {} micros", elapsed);
        if gas_estimate.is_ok() {
            println!("Gas estimate was {}", gas_estimate.unwrap());
        } else {
            println!("Gas estimate failed with error {}", gas_estimate.err().unwrap());
        }
        println!("Result = {}", hex::encode(&result.unwrap()));
    }
}
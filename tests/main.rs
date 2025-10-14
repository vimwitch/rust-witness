use num_bigint::BigInt;
use rust_witness::witness;
use std::collections::HashMap;
use std::time::Instant;
use std::str::FromStr;
use std::fs::File;
use std::io::{Read as _, BufReader};
use serde_json::Value;

fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
    let mut bits = Vec::new();
    for &byte in bytes {
        for j in 0..8 {
            let bit = (byte >> j) & 1;
            bits.push(bit == 1);
        }
    }
    bits
}

// Parse witness file format (.wtns)
// Format: "wtns" [version:4] [sections:4] [section1_id:4] [section1_len:8] [n8:4] [prime:n8] [nVars:4] [section2_id:4] [section2_len:8] [witness_values: nVars*n8]
fn parse_wtns_file(path: &str) -> Result<Vec<BigInt>, Box<dyn std::error::Error>> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    reader.read_to_end(&mut buffer)?;

    // Check magic "wtns"
    if &buffer[0..4] != b"wtns" {
        return Err("Invalid witness file: missing magic".into());
    }

    let mut pos = 4;

    // Read version (4 bytes)
    let _version = u32::from_le_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]);
    pos += 4;

    // Read number of sections (4 bytes)
    let _n_sections = u32::from_le_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]);
    pos += 4;

    // Section 1: Header
    let _section1_id = u32::from_le_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]);
    pos += 4;

    let _section1_len = u64::from_le_bytes([
        buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3],
        buffer[pos+4], buffer[pos+5], buffer[pos+6], buffer[pos+7]
    ]);
    pos += 8;

    // Read n8 (field element size in bytes)
    let n8 = u32::from_le_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]) as usize;
    pos += 4;

    // Skip prime (n8 bytes)
    pos += n8;

    // Read nVars (number of witness elements)
    let n_vars = u32::from_le_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]) as usize;
    pos += 4;

    // Section 2: Data
    let _section2_id = u32::from_le_bytes([buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3]]);
    pos += 4;

    let _section2_len = u64::from_le_bytes([
        buffer[pos], buffer[pos+1], buffer[pos+2], buffer[pos+3],
        buffer[pos+4], buffer[pos+5], buffer[pos+6], buffer[pos+7]
    ]);
    pos += 8;

    // Read witness values
    let mut witness = Vec::new();
    for _ in 0..n_vars {
        let mut bytes = vec![0u8; n8];
        bytes.copy_from_slice(&buffer[pos..pos+n8]);
        // Convert little-endian bytes to BigInt
        let val = BigInt::from_bytes_le(num_bigint::Sign::Plus, &bytes);
        witness.push(val);
        pos += n8;
    }

    Ok(witness)
}

#[cfg(test)]
witness!(keccak256256test);
#[cfg(test)]
witness!(multiplier2);
#[cfg(test)]
witness!(ecdsasecq256r1);
#[cfg(test)]
witness!(jwtsecq256r1);

#[test]
fn build_keccak_witness() {
    let input_vec = vec![
        116, 101, 115, 116, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0,
    ];

    let bits = bytes_to_bits(&input_vec);
    let big_int_bits = bits
        .into_iter()
        .map(|bit| BigInt::from(bit as u8))
        .collect();
    let mut inputs = HashMap::new();
    inputs.insert("in".to_string(), big_int_bits);

    let now = Instant::now();

    let _out = keccak256256test_witness(inputs);

    // TODO: verify the output

    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);
}

#[test]
fn build_multiplier2_witness() {
    let mut inputs = HashMap::new();
    inputs.insert("a".to_string(), vec![BigInt::from(3)]);
    inputs.insert("b".to_string(), vec![BigInt::from(11)]);

    let now = Instant::now();

    let out = multiplier2_witness(inputs);
    let elapsed = now.elapsed();
    println!("Elapsed: {:.2?}", elapsed);

    // For the multiplier2 circuit we input a = 3 and b = 11 and expect
    // the following witness data
    // 1, 33, 3, 11
    // The first witness entry is always 1. After this there are 3 values
    // defined in the circuit: the two inputs and one output and no intermediates

    assert_eq!(out[0], BigInt::from(1));
    assert_eq!(out[1], BigInt::from(33));
    assert_eq!(out[2], BigInt::from(3));
    assert_eq!(out[3], BigInt::from(11));
}

#[test]
fn build_ecdsa_secq256r1_witness() {
    let json_file = File::open("tests/ecdsa_secq256r1/ecdsa_input.json")
        .expect("Failed to open ecdsa_input.json");
    let json_value: Value = serde_json::from_reader(json_file)
        .expect("Failed to parse ecdsa_input.json");

    let mut inputs = HashMap::new();

    // Parse values from JSON
    inputs.insert(
        "s_inverse".to_string(),
        vec![BigInt::from_str(json_value["s_inverse"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "r".to_string(),
        vec![BigInt::from_str(json_value["r"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "m".to_string(),
        vec![BigInt::from_str(json_value["m"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "pubKeyX".to_string(),
        vec![BigInt::from_str(json_value["pubKeyX"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "pubKeyY".to_string(),
        vec![BigInt::from_str(json_value["pubKeyY"].as_str().unwrap()).unwrap()],
    );

    
    println!("Generating witness for ECDSA circuit with secp256r1 config...");
    let now = Instant::now();

    let witness = ecdsasecq256r1_witness(inputs);

    let elapsed = now.elapsed();
    println!("Witness generation took: {:.2?}", elapsed);
    println!("Witness size: {}", witness.len());

    let expected_witness = parse_wtns_file("tests/ecdsa_secq256r1/ecdsa.wtns")
        .expect("Failed to parse expected witness file");

    // First element should always be 1
    assert_eq!(witness[0], BigInt::from(1));
    assert_eq!(witness.len(), expected_witness.len(), "Witness size mismatch!");

    // Compare all witness values
    let mut mismatches = 0;
    for (i, (generated, expected)) in witness.iter().zip(expected_witness.iter()).enumerate() {
        if generated != expected {
            if mismatches < 10 {
                println!("  Mismatch at witness[{}]:", i);
                println!("    Generated: {}", generated);
                println!("    Expected:  {}", expected);
            }
            mismatches += 1;
        }
    }

    if mismatches > 0 {
        println!("\nTotal mismatches: {}/{}", mismatches, witness.len());
        panic!("Witness verification failed!");
    }

    println!("✅ All {} witness values match the expected witness!", witness.len());
}

#[test]
fn build_jwt_secq256r1_witness() {
    let json_file = File::open("tests/jwt_secq256r1/jwt_input.json")
        .expect("Failed to open jwt_input.json");
    let json_value: Value = serde_json::from_reader(json_file)
        .expect("Failed to parse jwt_input.json");

    let mut inputs = HashMap::new();

    inputs.insert(
        "sig_r".to_string(),
        vec![BigInt::from_str(json_value["sig_r"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "sig_s_inverse".to_string(),
        vec![BigInt::from_str(json_value["sig_s_inverse"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "pubKeyX".to_string(),
        vec![BigInt::from_str(json_value["pubKeyX"].as_str().unwrap()).unwrap()],
    );
    inputs.insert(
        "pubKeyY".to_string(),
        vec![BigInt::from_str(json_value["pubKeyY"].as_str().unwrap()).unwrap()],
    );

    // Parse message array
    let message_array = json_value["message"].as_array().unwrap();
    let message: Vec<BigInt> = message_array
        .iter()
        .map(|v| BigInt::from_str(v.as_str().unwrap()).unwrap())
        .collect();
    inputs.insert("message".to_string(), message);

    inputs.insert(
        "messageLength".to_string(),
        vec![BigInt::from(json_value["messageLength"].as_u64().unwrap())],
    );
    inputs.insert(
        "periodIndex".to_string(),
        vec![BigInt::from(json_value["periodIndex"].as_u64().unwrap())],
    );
    inputs.insert(
        "matchesCount".to_string(),
        vec![BigInt::from(json_value["matchesCount"].as_u64().unwrap())],
    );

    // Parse matchSubstring (2D array) - flatten it into 1D
    let match_substring_array = json_value["matchSubstring"].as_array().unwrap();
    let match_substring_flat: Vec<BigInt> = match_substring_array
        .iter()
        .flat_map(|inner_array| {
            inner_array
                .as_array()
                .unwrap()
                .iter()
                .map(|v| BigInt::from_str(v.as_str().unwrap()).unwrap())
        })
        .collect();
    inputs.insert("matchSubstring".to_string(), match_substring_flat);

    // Parse matchLength array
    let match_length: Vec<BigInt> = json_value["matchLength"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            if let Some(s) = v.as_str() {
                BigInt::from_str(s).unwrap()
            } else if let Some(n) = v.as_u64() {
                BigInt::from(n)
            } else {
                panic!("matchLength value must be string or number")
            }
        })
        .collect();
    inputs.insert("matchLength".to_string(), match_length);

    // Parse matchIndex array
    let match_index: Vec<BigInt> = json_value["matchIndex"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| BigInt::from(v.as_u64().unwrap()))
        .collect();
    inputs.insert("matchIndex".to_string(), match_index);

    // Parse claims (2D array) - flatten it into 1D
    let claims_array = json_value["claims"].as_array().unwrap();
    let claims_flat: Vec<BigInt> = claims_array
        .iter()
        .flat_map(|inner_array| {
            inner_array
                .as_array()
                .unwrap()
                .iter()
                .map(|v| BigInt::from_str(v.as_str().unwrap()).unwrap())
        })
        .collect();
    inputs.insert("claims".to_string(), claims_flat);

    // Parse claimLengths array
    let claim_lengths: Vec<BigInt> = json_value["claimLengths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| {
            if let Some(s) = v.as_str() {
                BigInt::from_str(s).unwrap()
            } else if let Some(n) = v.as_u64() {
                BigInt::from(n)
            } else {
                panic!("claimLengths value must be string or number")
            }
        })
        .collect();
    inputs.insert("claimLengths".to_string(), claim_lengths);

    // Parse decodeFlags array
    let decode_flags: Vec<BigInt> = json_value["decodeFlags"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| BigInt::from(v.as_u64().unwrap()))
        .collect();
    inputs.insert("decodeFlags".to_string(), decode_flags);

    println!("Generating witness for JWT circuit with secp256r1 config...");
    let now = Instant::now();

    let witness = jwtsecq256r1_witness(inputs);

    let elapsed = now.elapsed();
    println!("Witness generation took: {:.2?}", elapsed);
    println!("Witness size: {}", witness.len());

    // Load expected witness from file
    let expected_witness = parse_wtns_file("tests/jwt_secq256r1/jwt.wtns")
        .expect("Failed to parse expected JWT witness file");

    // First element should always be 1
    assert_eq!(witness[0], BigInt::from(1));
    assert_eq!(witness.len(), expected_witness.len(), "Witness size mismatch!");

    // Compare all witness values
    let mut mismatches = 0;
    for (i, (generated, expected)) in witness.iter().zip(expected_witness.iter()).enumerate() {
        if generated != expected {
            if mismatches < 10 {
                println!("  Mismatch at witness[{}]:", i);
                println!("    Generated: {}", generated);
                println!("    Expected:  {}", expected);
            }
            mismatches += 1;
        }
    }

    if mismatches > 0 {
        println!("\nTotal mismatches: {}/{}", mismatches, witness.len());
        panic!("JWT witness verification failed!");
    }

    println!("✅ All {} witness values match the expected witness!", witness.len());
}

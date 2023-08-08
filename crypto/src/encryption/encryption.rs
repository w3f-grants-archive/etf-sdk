use aes_gcm::{
    aead::{Aead, AeadCore, AeadInPlace, KeyInit, OsRng},
    Aes256Gcm, Nonce, // Or `Aes128Gcm`
};
use ark_std::rand::Rng;
use ark_bls12_381::Fr;
use ark_ff::{Zero, One, Field};
use ark_poly::{
    polynomial::univariate::DensePolynomial,
    DenseUVPolynomial, Polynomial,
};

pub struct AESOutput {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    // TODO: remove
    pub key: Vec<u8>,
}

pub enum Error {
    EncryptionError,
    DecryptionError,
    InvalidKey,
}

/// AES-GCM encryption of the message using an ephemeral keypair
/// basically a wrapper around the AEADs library to handle serialization
///
/// * `message`: The message to encrypt
///
pub fn aes_encrypt(message: &[u8], key: [u8;32]) -> Result<AESOutput, Error> {
    // let key = Aes256Gcm::generate_key(&mut OsRng);
    let cipher = Aes256Gcm::new(generic_array::GenericArray::from_slice(&key));
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message

    let mut buffer: Vec<u8> = Vec::new(); // Note: buffer needs 16-bytes overhead for auth tag
    buffer.extend_from_slice(message);
    // Encrypt `buffer` in-place, replacing the plaintext contents with ciphertext
    cipher.encrypt_in_place(&nonce, b"", &mut buffer)
        .map_err(|_| Error::EncryptionError)?;
    Ok(AESOutput{
        ciphertext: buffer,
        nonce: nonce.to_vec(),
        key: key.to_vec(),
    })
}

pub fn aes_decrypt(ciphertext: Vec<u8>, nonce_slice: &[u8], key: &[u8]) -> Result<Vec<u8>, Error> {
    // not sure about that...
    let cipher = Aes256Gcm::new_from_slice(key)
        .map_err(|_| Error::InvalidKey)?;
    let nonce = Nonce::from_slice(nonce_slice);
    let plaintext = cipher.decrypt(&nonce, ciphertext.as_ref())
        .map_err(|_| Error::DecryptionError)?;
    Ok(plaintext)
}

/// Generate a random polynomial f and return evalulations (f(0), (1, f(1), ..., n, f(n)))
/// f(0) is the 'secret' and the shares can be used to recover the secret with `let s = interpolate(shares);`
///
/// * `n`: The number of shares to generate
/// * `t`: The degree of the polynomial
/// * `rng`: A random number generator
///
pub fn generate_secrets<R: Rng + Sized>(
    n: u8, t: u8, mut rng: R) -> (Fr, Vec<(Fr, Fr)>) {
    let f = DensePolynomial::<Fr>::rand(t as usize, &mut rng);
    let msk = f.evaluate(&Fr::zero());
    let evals: Vec<(Fr, Fr)> = (1..n+1)
        .map(|i| {
            let e = Fr::from(i);
            (e, f.evaluate(&e))
        }).collect();
    (msk, evals)
}

/// interpolate a polynomial from the input and evaluate it at 0
///
/// * `evalulation`: a vec of (x, f(x)) pairs
///
pub fn interpolate(evaluations: Vec<(Fr, Fr)>) -> Fr {
    let n = evaluations.len();

    // Calculate the Lagrange basis polynomials evaluated at 0
    let mut lagrange_at_zero: Vec<Fr> = Vec::with_capacity(n);
    for i in 0..n {
        let mut basis_value = Fr::one();
        for j in 0..n {
            if i != j {
                let denominator = evaluations[i].0 - evaluations[j].0;
                basis_value *= denominator.inverse().unwrap() * evaluations[j].0;
            }
        }
        lagrange_at_zero.push(basis_value);
    }

    // Interpolate the value at 0
    let mut interpolated_value = Fr::zero();
    for i in 0..n {
        interpolated_value += evaluations[i].1 * lagrange_at_zero[i];
    }

    interpolated_value
}

#[cfg(test)]
mod test {
    use super::*;
    use ark_std::test_rng;

    #[test]
    pub fn aes_encrypt_decrypt_works() {
        let msg = b"test";
        match aes_encrypt(msg, [2;32]) {
            Ok(aes_out) => {
                match aes_decrypt(aes_out.ciphertext, &aes_out.nonce, &aes_out.key) {
                    Ok(plaintext) => {
                        assert_eq!(msg.to_vec(), plaintext);
                    }, 
                    Err(_) => {
                        panic!("test should pass");
                    }
                }
            },
            Err(_) => {
                panic!("test should pass");
            }
        }
    }

        
    #[test]
    fn secrets_interpolation() {
        let n = 5; // Number of participants
        let t = 3; // Threshold
        let (msk, shares) = generate_secrets(n, t, &mut test_rng());
        // Perform Lagrange interpolation
        let interpolated_msk = interpolate(shares);
        // Check if the msk and the interpolated msk match
        assert_eq!(msk, interpolated_msk);
    }
}
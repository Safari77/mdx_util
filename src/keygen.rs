use mdx::Result;

pub fn run_keygen(password: &str, id: &str) -> Result<()> {
    log::info!("Generating key for password and id");

    println!("Password: {}", password);
    let dict_cryto_key = mdx::crypto::digest::fast_hash_digest(password.as_bytes())?;
    println!("DB cipher: {:?}", hex::encode(&dict_cryto_key));
    println!("Identified by: {}", id);
    let ripemd_digest = mdx::crypto::digest::ripemd_digest(id.as_bytes())?;
    let reg_code = mdx::crypto::encryption::encrypt_salsa20(&dict_cryto_key, &ripemd_digest)?;
    println!("Reg code for end user: {:?}", hex::encode(&reg_code));

    Ok(())
}

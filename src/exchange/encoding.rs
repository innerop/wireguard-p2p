use std::io;
use std::io::Cursor;
use std::io::Read;

use bytes::BytesMut;
use bytes::BufMut;
use sodiumoxide::crypto::box_;
use sodiumoxide::crypto::pwhash;
use sodiumoxide::crypto::secretbox;

pub fn encode_key_and_psk(key: &secretbox::Key,
                          salt: &pwhash::Salt,
                          public_key: &box_::PublicKey,
                          local_psk: &[u8])
    -> Vec<u8>
{
    let nonce = secretbox::gen_nonce();

    let mut msg = BytesMut::with_capacity(512);
    msg.put("wg-p2p-key-v3");

    msg.put(&public_key[..]);
    msg.put(&local_psk[..]);

    let ciphertext = secretbox::seal(&msg[..], &nonce, &key);

    let mut value = BytesMut::with_capacity(512);
    value.put(&nonce[..]);
    value.put(&salt[..]);
    value.put(&ciphertext[..]);

    value.freeze().to_vec()
}

pub fn decode_key_and_psk(local_secret: &[u8],
                          remote_secret: &[u8],
                          value: Vec<u8>)
    -> Option<(box_::PublicKey, [u8; 32])>
{
    let res: io::Result<_> = try {
        let mut c = Cursor::new(value);

        let mut nonce = secretbox::Nonce::from_slice(&[0u8; secretbox::NONCEBYTES]).unwrap();
        let secretbox::Nonce(ref mut buf) = nonce;
        c.read_exact(buf)?;

        let mut salt = pwhash::Salt::from_slice(&[0u8; pwhash::SALTBYTES]).unwrap();
        let pwhash::Salt(ref mut buf) = salt;
        c.read_exact(buf)?;

        let mut ciphertext = vec![];
        c.read_to_end(&mut ciphertext)?;

        let mut shared_key = secretbox::Key::from_slice(&[0u8; secretbox::KEYBYTES]).unwrap();
        let secretbox::Key(ref mut kb) = shared_key;
        pwhash::derive_key(kb, &[remote_secret, local_secret].concat(), &salt,
                        pwhash::OPSLIMIT_INTERACTIVE,
                        pwhash::MEMLIMIT_INTERACTIVE).unwrap();

        let msg = secretbox::open(&ciphertext, &nonce, &shared_key);
        let msg = if let Ok(m) = msg {
            m
        } else {
            return None
        };
        let mut c = Cursor::new(msg);

        let expected = b"wg-p2p-key-v3";
        let mut actual = [0u8; 13];

        c.read_exact(&mut actual[..])?;

        let version_match = &actual == expected;

        let mut public_key = box_::PublicKey::from_slice(&[0u8; box_::PUBLICKEYBYTES]).unwrap();
        let box_::PublicKey(ref mut buf) = public_key;
        c.read_exact(buf)?;

        let mut psk = [0u8; 32];
        c.read_exact(&mut psk)?;

        if !version_match {
            return None;
        }

        (public_key, psk)
    };

    res.ok()
}

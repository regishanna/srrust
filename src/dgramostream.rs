//! Datagram over Stream : envoi et reception de datagrams sur une socket TCP bloquante.
//! Une entete representant la taille du datagram en big endian sur 16 bits est
//! inseree devant chaque datagram pour permettre sa reception en mode stream
//! 

use std::{io::{Read, Write}, net::TcpStream};


/// Envoi un datagram
pub fn send(mut sock: &TcpStream, buf: &[u8]) -> anyhow::Result<()> {
    // Envoi de l'entete contenant la taille du buffer en big endian
    let buf_len_bytes = u16::try_from(buf.len())?.to_be_bytes();
    let nb = sock.write(&buf_len_bytes)?;
    if nb < buf_len_bytes.len() {
        return Err(anyhow::anyhow!("Connexion fermee par le distant"));
    }

    // Envoi du buffer
    let nb = sock.write(buf)?;
    if nb < buf.len() {
        return Err(anyhow::anyhow!("Connexion fermee par le distant"));
    }

    Ok(())
}


/// Permet la reconstruction d'un datagram a partir de la lecture d'une socket TCP
pub struct RecvDgram {
    datagram: Vec<u8>,              // Buffer contenant le datagram
    datagram_cur_len: usize,        // Taille actuelle du buffer
    expected_len: Option<usize>,    // Taille prevue du buffer (connue grace a l'entete)
    header_buf: [u8; 2],            // Buffer contenant l'entete
    header_buf_cur_len: usize,      // Taille actuelle du buffer d'entete
}

impl RecvDgram {
    pub fn new(datagram_max_len: u16) -> RecvDgram {
        RecvDgram {
            datagram: vec![0; datagram_max_len as usize],
            datagram_cur_len: 0,
            expected_len: None,
            header_buf: [0; 2],
            header_buf_cur_len: 0,
        }
    }

    /// Efface un datagram en cours de reception
    pub fn clear(&mut self) {
        self.expected_len = None;
        self.header_buf_cur_len = 0;
    }

    /// Recoit un datagram
    /// Attention : en cas d'erreur, il faut faire appel a clear pour recevoir un nouveau datagram
    pub fn recv(&mut self, mut sock: &TcpStream) -> anyhow::Result<Option<&[u8]>> {
        // Doit-on recevoir le header ou le buffer ?
        match self.expected_len {
            None => {
                // On n'a pas totalement recu le header, on continue
                let nb = sock.read(&mut self.header_buf[self.header_buf_cur_len..])?;
                if nb == 0 {
                    Err(anyhow::anyhow!("Connexion fermee par le distant"))
                }
                else {
                    self.header_buf_cur_len += nb;
                    // A-t-on recu toute l'entete ?
                    if self.header_buf_cur_len >= self.header_buf.len() {
                        // Oui, on lit la taille attendue du datagram
                        let len = u16::from_be_bytes(self.header_buf) as usize;
                        if len > self.datagram.len() {
                            return Err(anyhow::anyhow!("Taille attendue du datagram ({}) superieure a la taille du buffer ({})", len, self.datagram.len()));
                        }
                        self.expected_len = Some(len);
                        self.datagram_cur_len = 0;
                    }
                    Ok(None)
                }
            },
            Some(expct_len) => {
                // On a deja recu le header, on recoit le buffer (ou on continue de le recevoir)
                let nb = sock.read(&mut self.datagram[self.datagram_cur_len..expct_len])?;
                if nb == 0 {
                    Err(anyhow::anyhow!("Connexion fermee par le distant"))
                }
                else {
                    self.datagram_cur_len += nb;
                    // A-t-on recu tout le buffer ?
                    if self.datagram_cur_len >= expct_len {
                        // Oui, c'est la fin de reception du datagram
                        self.clear();
                        Ok(Some(&self.datagram[..expct_len]))
                    }
                    else {
                        // Non, on devra rappeler la methode
                        Ok(None)
                    }
                }
            }
        }
    }

}

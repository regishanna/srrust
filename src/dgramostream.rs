//! Datagram over Stream: sending and receiving datagrams over a blocking TCP socket.
//! A header representing the size of the datagram in 16-bit big endian is
//! inserted in front of each datagram to allow its reception in stream mode
//! 

use std::{io::{Read, Write}, net::TcpStream};


/// Send a datagram
pub fn send(mut sock: &TcpStream, buf: &[u8]) -> anyhow::Result<()> {
    // Sending the header containing the size of the buffer in big endian
    let buf_len_bytes = u16::try_from(buf.len())?.to_be_bytes();
    sock.write_all(&buf_len_bytes)?;

    // Sending the buffer
    sock.write_all(buf)?;

    Ok(())
}


/// Allows the reconstruction of a datagram from reading a TCP socket
pub struct RecvDgram {
    datagram: Vec<u8>,              // Buffer containing the datagram
    datagram_cur_len: usize,        // Current buffer size
    expected_len: Option<usize>,    // Expected size of the buffer (known thanks to the header)
    header_buf: [u8; 2],            // Buffer containing the header
    header_buf_cur_len: usize,      // Current header buffer size
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

    /// Deletes a datagram being received
    pub fn clear(&mut self) {
        self.expected_len = None;
        self.header_buf_cur_len = 0;
    }

    /// Receives a datagram
    /// Warning: in the event of an error, you must call "clear" to receive a new datagram
    pub fn recv(&mut self, mut sock: &TcpStream) -> anyhow::Result<Option<&[u8]>> {
        // Should we receive the header or the buffer?
        match self.expected_len {
            None => {
                // We have not completely received the header, we continue
                let nb = sock.read(&mut self.header_buf[self.header_buf_cur_len..])?;
                if nb == 0 {
                    Err(anyhow::anyhow!("Connection closed by remote"))
                }
                else {
                    self.header_buf_cur_len += nb;
                    // Have we received all the header?
                    if self.header_buf_cur_len >= self.header_buf.len() {
                        // Yes, we read the expected size of the datagram
                        let len = u16::from_be_bytes(self.header_buf) as usize;
                        anyhow::ensure!(len <= self.datagram.len(),
                            "Expected size of the datagram ({}) greater than the size of the buffer ({})", len, self.datagram.len());
                        self.expected_len = Some(len);
                        self.datagram_cur_len = 0;
                    }
                    Ok(None)
                }
            },
            Some(expct_len) => {
                // We have already received the header, we receive the buffer (or we continue to receive it)
                let nb = sock.read(&mut self.datagram[self.datagram_cur_len..expct_len])?;
                if nb == 0 {
                    Err(anyhow::anyhow!("Connection closed by remote"))
                }
                else {
                    self.datagram_cur_len += nb;
                    // Have we received the entire buffer?
                    if self.datagram_cur_len >= expct_len {
                        // Yes, this is the end of datagram reception
                        self.clear();
                        Ok(Some(&self.datagram[..expct_len]))
                    }
                    else {
                        // No, we will have to recall the method
                        Ok(None)
                    }
                }
            }
        }
    }

}

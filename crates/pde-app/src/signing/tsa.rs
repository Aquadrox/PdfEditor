//! Transport HTTP vers un serveur d'horodatage RFC 3161 (S-08).
//!
//! Seule l'empreinte de la signature part vers le serveur ; sa réponse est
//! un jeton signé par lui, vérifié par `pde-sign` : HTTP suffit (c'est
//! l'usage des serveurs publics) et évite d'embarquer une pile TLS.

use std::io::Read;
use std::time::Duration;

use pde_core::settings::check_timestamp_url;
use pde_sign::TimestampAuthority;

/// Délai maximal d'une demande d'horodatage.
const TIMEOUT: Duration = Duration::from_secs(20);
/// Plus grande réponse acceptée (jeton et certificats du serveur).
const MAX_REPLY_BYTES: u64 = 256 * 1024;

pub struct HttpTimestampAuthority {
    url: String,
    agent: ureq::Agent,
}

impl HttpTimestampAuthority {
    /// Serveur à l'adresse `url` (`http://…`, vérifiée).
    pub fn new(url: &str) -> Result<Self, String> {
        check_timestamp_url(url)?;
        let agent = ureq::Agent::config_builder()
            .timeout_global(Some(TIMEOUT))
            .http_status_as_error(false)
            .build()
            .new_agent();
        Ok(Self {
            url: url.to_owned(),
            agent,
        })
    }
}

impl TimestampAuthority for HttpTimestampAuthority {
    fn request(&self, query: &[u8]) -> Result<Vec<u8>, String> {
        let started = std::time::Instant::now();
        let mut response = self
            .agent
            .post(&self.url)
            .header("Content-Type", "application/timestamp-query")
            .send(query)
            .map_err(|e| format!("serveur d'horodatage injoignable ({e})"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("le serveur d'horodatage répond {status}"));
        }
        let mut reply = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(MAX_REPLY_BYTES + 1)
            .read_to_end(&mut reply)
            .map_err(|e| format!("réponse du serveur d'horodatage interrompue ({e})"))?;
        if reply.len() as u64 > MAX_REPLY_BYTES {
            return Err("réponse du serveur d'horodatage trop grande".into());
        }
        log::info!(
            "horodatage obtenu en {} ms ({} octets)",
            started.elapsed().as_millis(),
            reply.len()
        );
        Ok(reply)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;

    use pde_sign::{Identity, LocalTimestampAuthority, Subject};

    use super::*;

    pub(crate) fn local_authority(time: u64) -> LocalTimestampAuthority {
        let identity = Identity::self_signed(
            &Subject {
                name: "Horodatage de test".into(),
                ..Subject::default()
            },
            time,
        )
        .unwrap();
        LocalTimestampAuthority::new(identity, time)
    }

    /// Serveur HTTP local d'un seul échange, qui répond comme un serveur d'horodatage.
    pub(crate) fn serve_once(authority: LocalTimestampAuthority) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0usize;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut query = vec![0; length];
            reader.read_exact(&mut query).unwrap();
            let reply = authority.request(&query).unwrap();
            let mut stream = stream;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/timestamp-reply\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                reply.len()
            )
            .unwrap();
            stream.write_all(&reply).unwrap();
        });
        format!("http://{address}/tsa")
    }

    #[test]
    fn unreachable_server_is_reported() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        let tsa = HttpTimestampAuthority::new(&format!("http://{address}/")).unwrap();
        let error = tsa.request(b"x").unwrap_err();
        assert!(error.contains("injoignable"), "{error}");
        assert!(HttpTimestampAuthority::new("https://exemple.fr").is_err());
    }
}

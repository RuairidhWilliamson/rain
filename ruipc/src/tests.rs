use std::io::{Read as _, Write as _};

use crate::{Client, Listener};

fn test_pipe_path(name: &str) -> String {
    #[cfg(target_family = "unix")]
    {
        let path = format!("/tmp/{name}");
        let _ = std::fs::remove_file(&path);
        path
    }
    #[cfg(target_family = "windows")]
    {
        format!("\\\\.\\pipe\\{name}")
    }
}

#[test]
fn create_server_client() {
    let path = test_pipe_path("server_client");
    let mut listener = Listener::bind(&path).unwrap();
    std::thread::scope(|s| {
        s.spawn(|| {
            let mut client = Client::connect(&path).unwrap();
            client.write_all(&[1, 2, 3, 4]).unwrap();
            let mut buf = vec![0u8; 4];
            client.read_exact(&mut buf).unwrap();
            assert_eq!(buf, vec![5, 6, 7, 8]);
        });
        let mut conn = listener.incoming().next().unwrap().unwrap();
        let mut buf = vec![0u8; 4];
        conn.read_exact(&mut buf).unwrap();
        assert_eq!(buf, vec![1, 2, 3, 4]);
        conn.write_all(&[5, 6, 7, 8]).unwrap();
    });
}

#[test]
fn server_multiple_clients() {
    let path = test_pipe_path("server_multiple_client");
    let mut listener = Listener::bind(&path).unwrap();
    std::thread::scope(|s| {
        s.spawn(|| {
            let mut client = Client::connect(&path).unwrap();
            client.write_all(&[1, 2, 3, 4]).unwrap();
            let mut buf = vec![0u8; 4];
            client.read_exact(&mut buf).unwrap();
            assert_eq!(buf, vec![5, 6, 7, 8]);
        });
        let mut conn = listener.incoming().next().unwrap().unwrap();
        let mut buf = vec![0u8; 4];
        conn.read_exact(&mut buf).unwrap();
        assert_eq!(buf, vec![1, 2, 3, 4]);
        conn.write_all(&[5, 6, 7, 8]).unwrap();

        s.spawn(|| {
            let mut client = Client::connect(&path).unwrap();
            client.write_all(&[10, 11, 12, 13]).unwrap();
            let mut buf = vec![0u8; 4];
            client.read_exact(&mut buf).unwrap();
            assert_eq!(buf, vec![9, 10, 11, 12]);
        });
        let mut conn = listener.incoming().next().unwrap().unwrap();
        let mut buf = vec![0u8; 4];
        conn.read_exact(&mut buf).unwrap();
        assert_eq!(buf, vec![10, 11, 12, 13]);
        conn.write_all(&[9, 10, 11, 12]).unwrap();
    });
}

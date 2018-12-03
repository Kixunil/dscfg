extern crate dscfg_client;
extern crate tokio;
extern crate serde_json;

fn print_help<P: AsRef<::std::path::Path>>(program_path: P) -> ! {
    println!("Usage: {} SOCKET (set KEY VALUE|listen KEY [KEYS...]|get KEY)", program_path.as_ref().display());
    println!();
    println!("Arguments:");
    println!("\tSOCKET         Unix socket to connect to.");
    println!("\tKEY            UTF-8 string identifying a setting.");
    println!("\tVALUE          JSON-encoded value. (Doesn't have to be an object.)");
    std::process::exit(1)
}

fn main() {
    use tokio::prelude::{Future, Stream};

    let mut args = std::env::args_os();
    let program_path = args.next().expect("Not even zeroth argument given");
    let socket_path = args.next().unwrap_or_else(|| print_help(&program_path));
    let operation = args.next().unwrap_or_else(|| print_help(&program_path));
    if operation == *"set" {
        let key = args
            .next()
            .unwrap_or_else(|| print_help(&program_path))
            .into_string()
            .unwrap_or_else(|_| { println!("Key isn't a UTF-8 string"); print_help(&program_path); });
        let value = args
            .next()
            .unwrap_or_else(|| print_help(&program_path))
            .into_string()
            .unwrap_or_else(|_| { println!("Value isn't a UTF-8 string"); print_help(&program_path); });

        let value = serde_json::from_str::<serde_json::Value>(&value)
            .unwrap_or_else(|err| { println!("Value isn't valid JSON: {}", err); print_help(&program_path); });

        let client = tokio::net::unix::UnixStream::connect(socket_path)
            .and_then(|client| {
                dscfg_client::new(client)
                    .set_value(key, value)
                    .map(std::mem::drop)
            })
            .or_else(|err| Ok(println!("Setting value failed: {:?}", err)));
        tokio::run(client);
    } else if operation == *"listen" {
        let key = args
            .next()
            .unwrap_or_else(|| print_help(&program_path))
            .into_string()
            .unwrap_or_else(|_| { println!("Key isn't a UTF-8 string"); print_help(&program_path); });

        let client = tokio::net::unix::UnixStream::connect(socket_path)
            .and_then(|client| {
                dscfg_client::new::<serde_json::Value, _>(client)
                    .listen_notifications(key, true)
                    .for_each(|(key, value)| {
                        println!("The value of {} changed to {}", key, value);
                        Ok(())
                    })
            })
            .or_else(|err| Ok(eprintln!("Waiting for notifications failed: {:?}", err)));
        tokio::run(client);
    } else if operation == *"get" {
        let key = args
            .next()
            .unwrap_or_else(|| print_help(&program_path))
            .into_string()
            .unwrap_or_else(|_| { println!("Key isn't a UTF-8 string"); print_help(&program_path); });

        let client = tokio::net::unix::UnixStream::connect(socket_path)
            .map_err(dscfg_client::ProtocolError::Communication)
            .and_then(|client| {
                dscfg_client::new::<serde_json::Value, _>(client)
                    .get_value(key)
                    .and_then(|(value, _)| {
                        serde_json::to_string(&value)
                              .map_err(Into::into)
                              .map_err(dscfg_client::ProtocolError::Communication)
                    })
                   .map(|value| println!("{}", value))
            })
            .or_else(|err| Ok(eprintln!("Getting value failed: {:?}", err)));
        tokio::run(client);
    } else {
        print_help(&program_path);
    }
    
}

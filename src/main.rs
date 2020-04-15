mod util;
mod ping;
mod packet;

use colored::*;

use clap::{App, AppSettings, Arg};

use std::thread;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ping::{Pinger, ReplyType};



fn main() {
    let matches = App::new("ring")
        .setting(AppSettings::ColoredHelp)
        .version("v1.0")
        .author("Bryan Becar <becar.bryan@gmail.com>")
        .about("A Rust clone of the `ping` utility.\nWritten for the Cloudflare 2020 Internship Application.\nThe name is a portmanteau of Rust and pING. :)")
        .arg(Arg::with_name("DESTINATION")
            .help("Hostname or IP adddress")
            .required(true)
            .index(1))
        .arg(Arg::with_name("timeout")
            .help("Set how long to wait for each pong before timing out (Default 5s)")
            .short("W")
            .takes_value(true))
        .arg(Arg::with_name("interval")
            .help("Set how long to wait in between ping (Default 1s)")
            .short("I")
            .takes_value(true))
        .arg(Arg::with_name("ttl")
            .help("Set ttl on outgoing packets")
            .short("t")
            .takes_value(true))
        .get_matches();
    
    let destination_host = matches.value_of("DESTINATION").unwrap();
    let destination = util::resolve_dest(destination_host).expect("Error resolving destination");

    let timeout = matches.value_of("timeout").unwrap_or("5s");
    let timeout = humantime::parse_duration(timeout).expect("Invalid duration for timeout (ex: 1s, 400ms, 1m) : ");

    let interval = matches.value_of("interval").unwrap_or("1s");
    let interval = humantime::parse_duration(interval).expect("Invalid duration for interval (ex: 1s, 400ms, 1m) : ");

    let ttl = matches.value_of("ttl").map(|ttl| ttl.parse::<u32>().expect("Invalid ttl: (ex: 64) : "));

    let mut pinger = Pinger::new(destination).expect("Error constructing pinger : ");
    if let Some(t) = ttl {
        pinger.set_ttl(t).expect("Error setting ttl : ");
    }

    println!("{} {} ({})", "PING".cyan(), destination_host.bold(), destination);

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");


    let mut lost_count = 0;
    let mut sent_count = 0;
    while running.load(Ordering::SeqCst) {
        let sequence_num = match pinger.ping() {
            Ok(n) => n,
            Err(e) => {
                eprintln!("Error sending ping: {}", e);
                thread::sleep(interval);
                continue;
            }
        };

        sent_count += 1;

        let pong = match pinger.receive_pong(sequence_num, timeout) {
            Ok(p) => p,
            Err(e) => {
                lost_count += 1;

                if let Some(code) = e.raw_os_error() {
                    if code == 11 {
                        println!("Ping timed out. Lost {}/{} ({}%)", 
                            lost_count.to_string().red().bold(), sent_count.to_string().bold(), 
                            format!("{:.2}", 100f32 * (lost_count as f32) / (sent_count as f32)).bold());
                        
                        thread::sleep(interval);
                        continue;
                    }
                }

                eprintln!("Error receiving pong: {}", e);
                thread::sleep(interval);
                continue;
            }
        };

        match pong.mtype {
            ReplyType::Reply => {
                print!("{} bytes from {} ({}): ",
                    pong.size, pong.hostname.or(Some(pong.address.to_string())).unwrap().yellow(), pong.address);
                
                print!("icmp_seq={} ", pong.sequence.to_string().bold());
        
                // Turns out it's really difficult to get the hop_limit from ipv6 packets because
                // the raw socket for ipv6 connections doesn't include the ipv6 header when it puts
                // the message into the buffer. (But it does put the ipv4 header in when the connection is ipv4)
                // Making this work would involve adding features to the socket2 crate to be able to use `recvmsg`
                if let Some(ttl) = pong.ttl {
                    print!("ttl={} ", ttl.to_string().bold());
                }

                print!("time={}ms ", format!("{:.2}", pong.rtt.as_micros() as f32 / 1000f32).bold());

                print!("loss={}%", format!("{:.2}", 100f32 * (lost_count as f32) / (sent_count as f32)).bold());

                println!(""); // Finish the line
            }

            ReplyType::TimeLimitExceeded => {
                print!("From {} ({}): ", pong.hostname.or(Some(pong.address.to_string())).unwrap(), pong.address);

                print!("icmp_seq={} ", pong.sequence);
                println!("Time to live exceeded");
                lost_count += 1; // TTL Timeout counts as a lost packet
            }
        }

        thread::sleep(interval);
    }

    println!(""); // New line
    println!("{} {} {} {}", "===".yellow(), destination_host.bold(), "ping statistics".cyan(), "===".yellow());
    println!("{} packets transmitted, {} received, {}% packet loss", 
        sent_count.to_string().bold(), (sent_count - lost_count).to_string().bold(), 
        format!("{:.2}", 100f32 * (lost_count as f32) / (sent_count as f32)).bold())
}

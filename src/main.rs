mod lab;
use std::{fs::{File, read_to_string}, io::{BufReader, BufRead, Write, Read, self}, error::Error, process::Stdio, time::{SystemTime, UNIX_EPOCH}, env};
use clap::{command, arg, ArgGroup, Command, Arg};

fn main(){
    let matches = cli().get_matches();

    let input = if matches.contains_id("request") {
        ("-request",  matches.get_one::<String>("request").unwrap())
    } else {
        ("-u", matches.get_one::<String>("url").unwrap())
    };

    let wordlist = matches.get_one::<String>("wordlist").unwrap();
    let time = matches.get_one::<String>("time").unwrap();
    let rate = matches.get_one::<String>("rate").unwrap();
    let number_of_parts = get_number_of_parts(wordlist, time, rate);
    let jwt_url = matches.try_get_one::<String>("jwt").unwrap();
    let extra_args = matches.get_many::<String>("args").unwrap_or_default().map(|v| v.as_str()).collect::<Vec<_>>();

    let args = vec![
        input.0, 
        input.1, 
        "-rate", 
        rate, 
        "-of", 
        "csv", 
    ];

    let final_args = [args, extra_args].concat();

    run(
        final_args, 
        (number_of_parts, jwt_url, wordlist)
    ).ok();
}

#[tokio::main]
async fn run(output: Vec<&str>, cfg: (i32, Option<&String>, &str)) -> io::Result<()> {
    let (number_of_parts, jwt_url, wordlist) = cfg;

    for part in 1..(number_of_parts + 1) {

        let jwt = if jwt_url.is_some() {
            format!("Authorization: {}", get_jwt(jwt_url).await.unwrap())
        } else {
            format!("")
        };

        let final_args = if jwt.len() > 1 {
            let header = vec!["-H", jwt.as_str()];
            let output = output.clone();
            let wordlist = vec!["-w", "-"];

            [header, output, wordlist].concat()
        } else {
            let output = output.clone();
            let wordlist = vec!["-w", "-"];

            [output, wordlist].concat()
        };

        let split = std::process::Command::new("split")
            .args([
                "-n", 
                format!("l/{}/{}", part, number_of_parts).as_str(), 
                wordlist
            ])
            .stdout(Stdio::piped())
            .spawn()?
            .stdout;

        let mut str = String::new();
        split.unwrap().read_to_string(&mut str).ok();

        let mut ffuf = match std::process::Command::new("ffuf")
            .args(&final_args)
            .args(get_output_file())
            .stdin(Stdio::piped())
            .spawn()
            {
                Err(why) => panic!("couldn't spawn: {}", why), 
                Ok(proc) => proc, 
            };

        match ffuf.stdin.as_ref().unwrap().write_all(str.as_bytes()) {
            Err(_) => panic!("could not split output to ffuf stdin"), 
            Ok(_) => ()
        }

        ffuf.wait().ok();
    }

    Ok(())
}

async fn get_jwt(url: Option<&String>) -> Result<String, Box<dyn Error>> {
    if url.is_none() { return Err("Empty jwt".into()); }

    let cookie_values: String = read_to_string("request.txt")
        .unwrap()
        .split("\n")
        .filter(|el| {
            el.starts_with("Cookie")
        })
        .map(|el| {
            let (_, cookie_values) = el.split_once(":").unwrap();
            cookie_values
        })
        .collect();

    assert!(!cookie_values.is_empty(), "cookie_values is empty");

    let req = reqwest::Client::builder()
        .build()?
        .get(url.unwrap())
        .header("Cookie", cookie_values)
        .send()
        .await?;

    assert_eq!(req.status(), 200, "jwt response returned different than 200");

    let res = req
        .text()
        .await?;

    assert!(!res.is_empty(), "jwt response returned empty");

    Ok(res)
}

fn get_number_of_parts(arg: &str, time: &String, rate: &String) -> i32 {
    let lines: i32 = BufReader::new(File::open(arg).unwrap())
        .split(b'\n')
        .count()
        .try_into()
        .expect("number of lines in wordlist is too big");

    let _time = time.parse::<i32>().unwrap();
    let _rate = rate.parse::<i32>().unwrap();

    let ret = lines / (_rate * 60 * _time) + 1;
    ret
}

fn cli() -> Command {
    command!()
        .group(ArgGroup::new("inputs").args(["request", "url"]).required(true))
        .args([
            arg!(-r --request <VALUE> "request file").aliases(["req"]), 
            arg!(-u --url <VALUE> "url http://...")
        ])
        .arg(arg!(-w --wordlist <VALUE> "wordlist file")
            .required(true))
        .arg(arg!(-t --time <VALUE>)
            .required(true)
        )
        .arg(arg!(--rate <VALUE>)
            .default_value("100")
        )
        .arg(Arg::new("args")
            .num_args(0..)
            .long("args")
            .short('a')
            .allow_hyphen_values(true)
        )
        .arg(arg!(--jwt <VALUE> "value of the url with the jwt token"))
        .after_help("--jwt option neds a file called request.txt with the cookies to fetch it")
}

fn get_output_file() -> Vec<String> {
    let home = env::var("HOME").unwrap();
    vec!["-o".to_string(), format!("{}/recon/results/jwt/_{:?}_.txt", home, SystemTime::now().duration_since(UNIX_EPOCH).unwrap())]
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_counts_right(){
        let lines = 1_000_000;
        let rate = 100;
        let time = 25;
        let number_of_parts = lines / (rate * 60 * time) + 1;
        assert_eq!(number_of_parts, 7);
    }

    #[test]
    fn it_counts_right_1_part(){
        let lines = 100;
        let rate = 100;
        let time = 25;
        let number_of_parts = lines / (rate * 60 * time) + 1;
        assert_eq!(number_of_parts, 1);
    }

    #[test]
    fn it_counts_right2(){
        let lines = 2_600_000;
        let rate = 100;
        let time = 1;
        let number_of_parts = lines / (rate * 60 * time) + 1;
        assert_eq!(number_of_parts, 14);
    }
}

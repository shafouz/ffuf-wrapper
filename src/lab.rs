use crate::cli;

#[allow(unused)]
fn main(){
    let matches = cli().get_matches();
    let args = matches.get_one::<Vec<String>>("args").unwrap();
}

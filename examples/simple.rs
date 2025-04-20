use berry::parse::parse_lockfile;
fn main() {
  let file_contents = include_str!("../fixtures/berry.lock");
  let result = parse_lockfile(file_contents);
  println!("{result:?}");
}

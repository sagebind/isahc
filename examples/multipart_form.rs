use isahc::{forms::FormDataBuilder, prelude::*, Body};

fn main() -> Result<(), isahc::Error> {
    let form = FormDataBuilder::<Body>::new().field("foo", "bar").build();

    let mut response = isahc::post("https://httpbin.org/post", form)?;

    println!("{:?}", response);
    print!("{}", response.text()?);

    Ok(())
}

use warp::{Filter, Reply, Rejection};
use warp::reject::Reject;
use warp::http::{Response, StatusCode};
use rusttype::Font;
use image::io::Reader as ImageReader;
use image::RgbaImage;
use std::path::Path;

const DEJA_VU_FONT: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

#[derive(Debug)]
struct InvalidBody;

impl Reject for InvalidBody{}

async fn upload(body: bytes::Bytes) -> Result<impl warp::Reply, Rejection> {
    let req = String::from_utf8(body.to_vec())
        .map_err(|e| warp::reject::custom(InvalidBody))?;
    let req = req.trim();
    let words: Vec<_> = req.lines().collect();
    if words.len() < 25 {
        return Err(warp::reject::custom(InvalidBody));
    }
    // and now generate the image
    // TODO: we shouldn't be loading this font from disk every time, lazy_static it
    let font = Font::try_from_vec(Vec::from(DEJA_VU_FONT)).unwrap();
    // For unknown reasons, ImageReader::new(Cursor::new(include_bytes!(../assets/file.png)))
    // does not decode correctly, so don't compile the image into the binary :(
    // TODO: we shouldn't be loading these images from disk every time
    let front = ImageReader::open("./assets/codenames-front.png").unwrap().decode().unwrap();
    let front = front.as_rgba8().unwrap();
    let back = ImageReader::open("./assets/codenames-back.png").unwrap().decode().unwrap();
    let back = back.as_rgba8().unwrap();

    let num_cards = words.len();

    let width = front.width();
    let height = front.height();
    let out_height = height * num_cards as u32;

    let mut out_f = RgbaImage::new(width, out_height);
    let mut out_b = RgbaImage::new(width, out_height);
    image::imageops::tile(&mut out_f, front);
    image::imageops::tile(&mut out_b, back);
    // And now write on the words
    // hack: hardcode where the text box is on my cards
    // This makes using generic width/height above quite silly, but no matter.
    let text_rect = (40, 134, 313, 190);

    for (i, word) in words.iter().enumerate() {
        // TODO: calculate the width of the drawn glyphs before drawing them so we can center them.
        // Also, reduce the scale size down if it's overflowing the box based on the pre-draw math.
        imageproc::drawing::draw_text_mut(
            &mut out_f,
            *image::Pixel::from_slice(&[0 as u8, 0, 0, 255]),
            text_rect.0,
            text_rect.1 + (i as u32 * height),
            rusttype::Scale::uniform(64.0),
            &font,
            word,
        );
    }


    // And now write the image
    let id = uuid::Uuid::new_v4();
    std::fs::create_dir_all("decks").unwrap();
    out_f.save(Path::new("decks").join(id.to_string() + "_f.png")).unwrap();
    out_b.save(Path::new("decks").join(id.to_string() + "_b.png")).unwrap();
    // TODO: json
    Ok(Response::builder().body(id.to_string()))
}

#[tokio::main]
async fn main() {
    let root = warp::path::end().map(|| "Welcome to this AGPL licensed webpage. Source code is at /source.tar.gz");
    let upload = warp::path("upload").and(warp::filters::body::bytes()).and_then(upload);
    let deck = warp::path("deck").map(|| "Hello, World!");

    let routes = warp::get().and(
        deck
        .or(root),
    ).or(
        warp::post().and(upload)
    );
    let routes = routes.recover(handle_rejection);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    // TODO: bother with error handling
    Ok(warp::reply::with_status(":(", StatusCode::INTERNAL_SERVER_ERROR))
}

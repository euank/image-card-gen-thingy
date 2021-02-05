use image::io::Reader as ImageReader;
use image::RgbaImage;
use include_repo::*;
use rusttype::Font;
use serde::Serialize;
use std::path::Path;
use warp::http::{Response, StatusCode};
use warp::reject::Reject;
use warp::{Filter, Rejection, Reply};

const DEJA_VU_FONT: &[u8] = include_bytes!("../assets/DejaVuSans.ttf");

include_repo::include_repo!(SOURCE_CODE);

#[derive(Debug)]
struct InvalidBody;

impl Reject for InvalidBody {}

#[derive(Debug, Serialize)]
struct Cards {
    front: String,
    back: String,
    num_cards: u32,
    num_cards_wide: u32,
    num_cards_tall: u32,
}

async fn upload(conf: Config, body: bytes::Bytes) -> Result<impl warp::Reply, Rejection> {
    let req = String::from_utf8(body.to_vec()).map_err(|e| warp::reject::custom(InvalidBody))?;
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
    let front = ImageReader::open("./assets/codenames-front.png")
        .unwrap()
        .decode()
        .unwrap();
    let front = front.as_rgba8().unwrap();
    let back = ImageReader::open("./assets/codenames-back.png")
        .unwrap()
        .decode()
        .unwrap();
    let back = back.as_rgba8().unwrap();

    let num_cards = words.len();
    // Make it as square as possible because TTS gets mad at images with alrge width or height
    // values, and squaring it up minimizes those.
    let card_w = (num_cards as f64).sqrt().floor() as u32;
    let card_h = (num_cards as f64 / card_w as f64).ceil() as u32;
    let width = front.width();
    let height = front.height();
    let out_width = width * card_w as u32;
    let out_height = height * card_h as u32;

    let mut out_f = RgbaImage::new(out_width, out_height);
    let mut out_b = RgbaImage::new(out_width, out_height);
    image::imageops::tile(&mut out_f, front);
    image::imageops::tile(&mut out_b, back);
    // And now write on the words
    // hack: hardcode where the text box is on my cards
    // This makes using generic width/height above quite silly, but no matter.
    let text_rect = (40, 134, 313, 190);

    for (i, word) in words.iter().enumerate() {
        let x_off = i as u32 % card_w;
        let y_off = i as u32 / card_w;
        // TODO: calculate the width of the drawn glyphs before drawing them so we can center them.
        // Also, reduce the scale size down if it's overflowing the box based on the pre-draw math.
        imageproc::drawing::draw_text_mut(
            &mut out_f,
            *image::Pixel::from_slice(&[0 as u8, 0, 0, 255]),
            text_rect.0 + (x_off * width),
            text_rect.1 + (y_off * height),
            rusttype::Scale::uniform(64.0),
            &font,
            word,
        );
    }

    // And now write the image
    let id = uuid::Uuid::new_v4();
    std::fs::create_dir_all("decks").unwrap();
    out_f
        .save(Path::new("decks").join(id.to_string() + "_f.png"))
        .unwrap();
    out_b
        .save(Path::new("decks").join(id.to_string() + "_b.png"))
        .unwrap();
    Ok(warp::reply::json(&Cards {
        front: format!("{}/deck/{}_f.png", conf.root, id),
        back: format!("{}/deck/{}_b.png", conf.root, id),
        num_cards: num_cards as u32,
        num_cards_wide: card_w as u32,
        num_cards_tall: card_h as u32,
    }))
}

#[derive(Clone, Debug)]
struct Config {
    root: String,
}

impl Config {
    fn must_from_env() -> Self {
        Config {
            root: std::env::var("ROOT")
                .expect("Must set ROOT env var to the url this website is served at"),
        }
    }
}

fn config(
    conf: Config,
) -> impl Filter<Extract = (Config,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || conf.clone())
}

#[tokio::main]
async fn main() {
    let conf = Config::must_from_env();

    let root = warp::path::end()
        .map(|| "Welcome to this AGPL licensed webpage. Source code is at /source");
    let upload = warp::path("upload")
        .and(config(conf))
        .and(warp::filters::body::bytes())
        .and_then(upload);
    let decks = warp::path("deck").and(warp::fs::dir("./decks"));

    let source_code = warp::path("source").map(|| {
        Response::builder()
            .header("Content-Type", "application/x-tar")
            .body(&SOURCE_CODE[..])
    });

    let routes = warp::get()
        .and(root.or(decks).or(source_code))
        .or(warp::put().and(upload));
    let routes = routes.recover(handle_rejection);
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}

async fn handle_rejection(err: Rejection) -> Result<impl Reply, std::convert::Infallible> {
    // TODO: bother with error handling
    println!("error: {:?}", err);
    Ok(warp::reply::with_status(
        ":(",
        StatusCode::INTERNAL_SERVER_ERROR,
    ))
}

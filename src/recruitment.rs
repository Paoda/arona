use crate::general::BLUE_ARCHIVE_BLUE;
use crate::image::get_image_from_url;
use bluearch_recruitment::banner::{Banner, BannerBuilder};
use bluearch_recruitment::gacha::Recruitment as RecruitmentTrait;
use bluearch_recruitment::gacha::{GachaBuilder, Rarity};
use bluearch_recruitment::i18n::Language;
use bluearch_recruitment::student::Student;
use image::jpeg::JpegEncoder;
use image::{ColorType, ImageEncoder, RgbaImage};
use lazy_static::lazy_static;
use log::{error, info};
use serenity::client::Context;
use serenity::framework::standard::CommandResult;
use serenity::model::channel::Message;
use serenity::utils::Colour;

use std::time::Instant;

const STUDENTS_JSON: &str = include_str!("../data/students.json");
const CDN_URL: &str = "https://rerollcdn.com/BlueArchive";
const BANNER_IMG_URL: &str = "https://static.wikia.nocookie.net/blue-archive/images/e/e0/Gacha_Banner_01.png/revision/latest/";
const THUMB_WIDTH: u32 = 202; // OG: 404 (2020-02-11) from https://thearchive.gg
const THUMB_HEIGHT: u32 = 228; // OG: 456 (2020-02-11) from https://thearchive.gg

lazy_static! {
    static ref STUDENTS: Vec<Student> = serde_json::from_str(STUDENTS_JSON).unwrap();
    static ref BANNER: Banner = create_banner();
}

pub async fn roll(ctx: &Context, msg: &Message) -> CommandResult {
    let author_name = format!("{}#{}", msg.author.name, msg.author.discriminator);
    info!("{} requested a single roll", author_name);

    let channel = msg.channel_id;
    let student = BANNER.roll();

    let eng_name = student.name.get(Language::English).unwrap();
    let url_name = if eng_name == "Junko" {
        "Zunko"
    } else {
        &eng_name
    };

    let img_url = format!("{}/Characters/{}.png", CDN_URL, url_name);
    let title_url = format!("https://www.thearchive.gg/characters/{}", url_name);
    let icon_url = format!("{}/Icons/icon-brand.png", CDN_URL);
    let rarity_colour = get_rarity_colour(student.rarity);

    let rarity_str = match student.rarity {
        Rarity::One => ":star:",
        Rarity::Two => ":star::star:",
        Rarity::Three => ":star::star::star:",
    };

    channel
        .send_message(ctx, |m| {
            m.embed(|embed| {
                embed
                    .image(img_url)
                    .title(format!("{}", student.name))
                    .description(format!("{}\t{}", eng_name, rarity_str))
                    .url(title_url)
                    .footer(|footer| {
                        footer
                            .icon_url(icon_url)
                            .text("Image Source: https://thearchive.gg")
                    })
                    .colour(rarity_colour)
            })
        })
        .await?;

    Ok(())
}

pub async fn roll10(ctx: &Context, msg: &Message) -> CommandResult {
    let author_name = format!("{}#{}", msg.author.name, msg.author.discriminator);
    info!("{} requested a ten roll", author_name);
    let channel = msg.channel_id;

    let typing = channel.start_typing(&ctx.http)?;

    const IMG_WIDTH: u32 = THUMB_WIDTH * 5;
    const IMG_HEIGHT: u32 = THUMB_HEIGHT * 2;

    let mut collage = RgbaImage::new(IMG_WIDTH, IMG_HEIGHT);
    let mut images: Vec<RgbaImage> = Vec::with_capacity(10);

    let students = BANNER.roll10();
    let mut max_rarity = Rarity::One;

    let start = Instant::now();
    for student in students.iter() {
        let eng_name = student.name.get(Language::English).unwrap();
        let url_name = if eng_name == "Junko" {
            "Zunko"
        } else {
            &eng_name
        };

        max_rarity = max_rarity.max(student.rarity);

        let img_url = format!("{}/Characters/{}.png", CDN_URL, url_name);
        let image = get_image_from_url(&img_url, THUMB_WIDTH, THUMB_HEIGHT).await;

        images.push(image);
    }
    let elapsed_ms = (Instant::now() - start).as_millis();
    info!("10-roll, DL, and resize took {}ms", elapsed_ms);

    let start = Instant::now();
    for x in (0..IMG_WIDTH).step_by(THUMB_WIDTH as usize) {
        let index: usize = (((IMG_WIDTH - x) / THUMB_WIDTH) - 1) as usize;

        // Top Image
        image::imageops::overlay(&mut collage, &images[index], x, 0);

        // Bottom Image
        image::imageops::overlay(&mut collage, &images[index + 5], x, THUMB_HEIGHT);
    }
    let elapsed_ms = (Instant::now() - start).as_millis();
    info!("Collage Build took {}ms", elapsed_ms);

    let mut jpeg = Vec::new();
    let encoder = JpegEncoder::new(&mut jpeg);

    let write_result = encoder.write_image(&collage, IMG_WIDTH, IMG_HEIGHT, ColorType::Rgba8);

    if let Err(err) = write_result {
        error!("Failed to Encode JPEG: {:?}", err);
        msg.reply(
            ctx,
            "アロナ failed to perform your 10-roll. Please try again",
        )
        .await?;
        return Ok(());
    }

    let icon_url = format!("{}/Icons/icon-brand.png", CDN_URL);

    let files = vec![(jpeg.as_slice(), "result.jpeg")];
    let _ = typing.stop();
    channel
        .send_files(ctx, files, |m| {
            m.embed(|embed| {
                embed
                    .title(format!("{} 10-roll", BANNER.name))
                    .description(BANNER.name.get(Language::English).unwrap())
                    .attachment("result.jpeg")
                    .colour(get_rarity_colour(max_rarity))
                    .footer(|footer| {
                        footer
                            .icon_url(icon_url)
                            .text("Image Source: https://thearchive.gg")
                    })
            })
        })
        .await?;
    Ok(())
}

pub async fn banner(ctx: &Context, msg: &Message) -> CommandResult {
    let author_name = format!("{}#{}", msg.author.name, msg.author.discriminator);
    info!("{} requested banner information", author_name);

    let channel = msg.channel_id;
    let banner_eng = BANNER.name.get(Language::English).unwrap();

    channel
        .send_message(ctx, |m| {
            m.embed(|embed| {
                embed
                    .image(BANNER_IMG_URL)
                    .title(BANNER.name.clone())
                    .description(banner_eng)
                    .colour(BLUE_ARCHIVE_BLUE)
            })
        })
        .await?;

    Ok(())
}

pub fn create_banner() -> Banner {
    let pool: Vec<Student> = STUDENTS
        .iter()
        .filter(|student| student.name != "ノゾミ")
        .cloned()
        .collect();

    let sparkable: Vec<Student> = pool
        .iter()
        .filter(|student| student.name == "ホシノ" || student.name == "シロコ")
        .cloned()
        .collect();

    let gacha = GachaBuilder::new(79.0, 18.5, 2.5)
        .with_pool(pool)
        .with_priority(&sparkable, 0.7)
        .finish()
        .unwrap();

    BannerBuilder::new("ピックアップ募集")
        .with_gacha(&gacha)
        .with_name_translation(Language::English, "Rate-Up Recruitment")
        .with_sparkable_students(&sparkable)
        .finish()
        .unwrap()
}

fn get_rarity_colour(rarity: Rarity) -> Colour {
    match rarity {
        Rarity::One => Colour::from_rgb(227, 234, 240),
        Rarity::Two => Colour::from_rgb(255, 248, 124),
        Rarity::Three => Colour::from_rgb(253, 198, 229),
    }
}
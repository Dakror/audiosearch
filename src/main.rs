use image::{GrayImage, ImageBuffer, Luma};
use std::{f32::consts::PI, fs::File, io::ErrorKind};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    dsp::{complex::Complex, fft::Fft},
    errors::Error,
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
};

fn main() -> anyhow::Result<()> {
    if !cfg!(target_os = "windows") {
        println!("Only supporting windoof right now lol");
        anyhow::bail!("Unsupported OS");
    }

    // let path = "./piano2.wav";
    let path = "./500hz.wav";
    // let path = "./200hz+500hz.wav";
    // let path = "./output.wav";
    /*
        if !std::fs::metadata(path).is_ok() {
            let res = Command::new("./yt-dlp")
                .args([
                    "-x",
                    "--audio-format",
                    "wav",
                    "-o",
                    path,
                    "https://www.youtube.com/watch?v=sZJjVpU3hL0",
                ])
                .output()
                .expect("failed to execute process");

            println!(
                "Response: {}",
                String::from_utf8(res.stdout).unwrap_or("err".to_owned())
            );
        }
    */
    // Open the media source.
    let src = File::open(path).expect("failed to open media");

    // Create the media source stream.
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Find the first audio track with a known (decodeable) codec.
    let fmt_opts: FormatOptions = Default::default();

    let mut format = symphonia::default::formats::WavReader::try_new(mss, &fmt_opts).expect("Could not read file");

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;

    let mut channels: Vec<Vec<Complex>> = Vec::new();

    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(Error::ResetRequired) => {
                // The track list has been changed. Re-examine it and create a new set of decoders,
                // then restart the decode loop. This is an advanced feature and it is not
                // unreasonable to consider this "the end." As of v0.5.0, the only usage of this is
                // for chained OGG physical streams.
                unimplemented!();
            }
            Err(err) => {
                if let Error::IoError(ref er) = err {
                    // Catch eof, not sure how to do it properly
                    if er.kind() == ErrorKind::UnexpectedEof {
                        break;
                    }
                }

                // A unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        };

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                // Consume the decoded audio samples
                let channel_count = audio_buf.spec().channels.count();
                let frames = audio_buf.frames();

                let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, *audio_buf.spec());
                sample_buf.copy_interleaved_ref(audio_buf);

                if channels.is_empty() {
                    for _ in 0..channel_count {
                        channels.push(Vec::with_capacity(frames));
                    }
                }

                for i in sample_buf.samples().chunks(channel_count) {
                    for j in 0..channel_count {
                        channels[j].push(Complex::new(i[j], 0.));
                    }
                }
            }
            Err(Error::IoError(_)) => {
                // The packet failed to decode due to an IO error, skip the packet.
                continue;
            }
            Err(Error::DecodeError(_)) => {
                // The packet failed to decode due to invalid data, skip the packet.
                continue;
            }
            Err(err) => {
                // An unrecoverable error occurred, halt decoding.
                panic!("{}", err);
            }
        }
    }

    let chunk_size = 2048;

    let fft = Fft::new(chunk_size);
    let mut values: Vec<Complex> = Vec::new();
    values.resize(fft.size(), Complex::new(0., 0.));

    let left = &channels[0];

    let samples = left.len() / chunk_size;

    let mut img: GrayImage = ImageBuffer::new(samples as u32, chunk_size as u32);
    img.fill(0);

    // hann window
    let window: Vec<f32> = (0..chunk_size)
        .map(|x| 0.5 * (1.0 - f32::cos(2.0 * (PI as f32) * (x as f32) / (chunk_size - 1) as f32)))
        .collect();

    for i in 0..samples {
        let x = i * chunk_size;
        if x + chunk_size > left.len() {
            break;
        }

        // load values and apply window function
        for i in 0..chunk_size {
            values[i] = left[x + i] * window[i];
        }

        fft.fft_inplace(&mut values);

        let luma: Vec<f32> = values.iter().map(|v| (v.re * v.re + v.im * v.im).sqrt()).collect();
        let max = luma.iter().reduce(|x, y| if x > y { x } else { y }).unwrap();

        for j in 0..chunk_size {
            let v = luma[j];
            img.put_pixel(i as u32, (chunk_size - j - 1) as u32, Luma([((v / max) * 255.) as u8]));
        }

        // let mut out_file = File::create("./fft2.csv")?;
        // for j in 0..chunk_size {
        //     out_file.write_fmt(format_args!("{};{}\n", values[j].re, values[j].im))?;
        // }
    }

    img.save("./spectrum.png")?;

    Ok(())
}

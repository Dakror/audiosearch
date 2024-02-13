use std::{
    fs::File,
    io::{self, ErrorKind, Write},
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    dsp::{complex::Complex, fft::Fft},
    errors::Error,
    formats::{FormatOptions, FormatReader},
    io::MediaSourceStream,
};

fn main() -> std::io::Result<()> {
    if !cfg!(target_os = "windows") {
        println!("Only supporting windoof right now lol");
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "Unsupported OS".to_owned(),
        ));
    }

    let path = "./200hz+500hz-stereo.wav";
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

    let mut left: Vec<Complex> = Vec::new();
    let mut right: Vec<Complex> = Vec::new();

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
                println!(
                    "Sample {} {} {}",
                    audio_buf.spec().rate,
                    audio_buf.frames(),
                    audio_buf.capacity()
                );

                let frames = audio_buf.frames();

                let mut sample_buf = SampleBuffer::<f32>::new(audio_buf.capacity() as u64, *audio_buf.spec());
                sample_buf.copy_interleaved_ref(audio_buf);

                left.reserve(frames);
                right.reserve(frames);
                for i in sample_buf.samples().windows(2) {
                    left.push(Complex::new(i[0], 0.));
                    right.push(Complex::new(i[1], 0.));
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

    let fft = Fft::new(4096);

    let mut output: Vec<Complex> = Vec::new();
    output.resize(fft.size(), Complex::new(0., 0.));
    fft.fft(&left[0..fft.size()], &mut output);

    let mut out_file = File::create("./fft.csv")?;
    for i in 0..2048 {
        out_file.write_fmt(format_args!("{};{}\n", output[i].re, output[i].im))?;
    }

    Ok(())

    //     println!("What's your name?");
    //     let mut buffer = String::new();
    //
    //     if let Ok(_) = io::stdin().read_line(&mut buffer) {
    //         println!("Hello, {}", buffer);
    //     } else {
    //         println!("Who are you?!")
    //     }

    //     let output = YoutubeDl::new("https://www.youtube.com/watch?v=sZJjVpU3hL0")
    //         .socket_timeout("15")
    //         .youtube_dl_path("./yt-dlp.exe")
    //         .run();
    //
    //     if let Ok(json) = output {
    //         if let Some(video) = json.into_single_video() {
    //             println!(
    //                 "Video title: {}",
    //                 video.title.unwrap_or("No title".to_owned())
    //             );
    //
    //             // Perform a forward FFT of size 1234
    //             let mut planner = FftPlanner::<f32>::new();
    //             let fft = planner.plan_fft_forward(1234);
    //
    //             let mut buffer = vec![Complex { re: 0.0, im: 0.0 }; 1234];
    //
    //             fft.process(&mut buffer);
    //         }
    //     }
}

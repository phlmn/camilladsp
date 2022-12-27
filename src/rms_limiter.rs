use crate::config;
use crate::filters::Filter;
use ringbuffer::{AllocRingBuffer, RingBuffer, RingBufferExt, RingBufferWrite};

use crate::PrcFmt;
use crate::Res;

pub struct RMSLimiter {
    pub name: String,
    samplerate: usize,
    chunksize: usize,
    rms_buffer: AllocRingBuffer<PrcFmt>,
    threshold_voltage_ratio: PrcFmt,
    decay_per_chunk: PrcFmt,
    current_gain: PrcFmt,
}

impl RMSLimiter {
    pub fn from_config(
        name: &str,
        conf: config::RMSLimiterParameters,
        chunksize: usize,
        samplerate: usize,
    ) -> Self {
        let decay_per_chunk = RMSLimiter::decay_per_chunk(chunksize, samplerate, &conf);
        let threshold_voltage_ratio = RMSLimiter::db_to_voltage_ratio(conf.threshold as PrcFmt);
        let rms_buffer = AllocRingBuffer::with_capacity(conf.rms_samples);

        RMSLimiter {
            name: name.to_string(),
            samplerate,
            chunksize,
            rms_buffer,
            threshold_voltage_ratio,
            current_gain: 1.0,
            decay_per_chunk,
        }
    }

    fn decay_per_chunk(
        chunksize: usize,
        samplerate: usize,
        conf: &config::RMSLimiterParameters,
    ) -> PrcFmt {
        let decay_db_per_chunk =
            conf.decay * RMSLimiter::chunks_per_second(chunksize, samplerate) as f32;
        RMSLimiter::db_to_voltage_ratio(decay_db_per_chunk as PrcFmt)
    }

    fn chunks_per_second(chunksize: usize, samplerate: usize) -> f32 {
        chunksize as f32 / samplerate as f32
    }

    fn db_to_voltage_ratio(db: PrcFmt) -> PrcFmt {
        (10.0 as PrcFmt).powf(db / 20.0)
    }

    fn voltage_ratio_to_db(voltage_ratio: PrcFmt) -> PrcFmt {
        20.0 * voltage_ratio.log10()
    }

    fn rms<'a>(waveform: impl Iterator<Item = &'a PrcFmt>) -> PrcFmt {
        let mut squared_sum: PrcFmt = 0.0;
        let mut values: u32 = 0;

        for item in waveform {
            squared_sum += item * item;
            values += 1;
        }

        (squared_sum / values as PrcFmt).sqrt()
    }
}

impl Filter for RMSLimiter {
    fn name(&self) -> &str {
        &self.name
    }

    fn process_waveform(&mut self, waveform: &mut [PrcFmt]) -> Res<()> {
        for item in waveform.iter_mut() {
            self.rms_buffer.push(*item)
        }
        
        let rms = RMSLimiter::rms(self.rms_buffer.iter());

        let gain = self.threshold_voltage_ratio / rms;
        let gain = PrcFmt::min(1.0, gain);

        if gain < self.current_gain {
            self.current_gain = gain;
        } else {
            self.current_gain = PrcFmt::min(1.0, self.current_gain * self.decay_per_chunk);
        }

        if self.current_gain < 1.0 {
            debug!(
                "Limiting by {:.2} db",
                RMSLimiter::voltage_ratio_to_db(self.current_gain)
            );
        }

        for item in waveform.iter_mut() {
            *item *= self.current_gain;
        }

        Ok(())
    }

    fn update_parameters(&mut self, conf: config::Filter) {
        if let config::Filter::RMSLimiter { parameters: conf, .. } = conf {
            self.decay_per_chunk = RMSLimiter::decay_per_chunk(self.chunksize, self.samplerate, &conf);
            self.threshold_voltage_ratio = RMSLimiter::db_to_voltage_ratio(conf.threshold as PrcFmt);

            if self.rms_buffer.capacity() != conf.rms_samples {
                self.rms_buffer = AllocRingBuffer::with_capacity(conf.rms_samples);
            }
        } else {
            // This should never happen unless there is a bug somewhere else
            panic!("Invalid config change!");
        }
    }
}

/// Validate a RMSLimiter config.
pub fn validate_config(conf: &config::RMSLimiterParameters) -> Res<()> {
    if conf.decay < 0.0 {
        return Err(config::ConfigError::new("Decay (dB/s) cannot be negative").into());
    }
    Ok(())
}

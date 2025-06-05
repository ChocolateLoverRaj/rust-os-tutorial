use bincode::{Decode, Encode};
use zerocopy::IntoBytes;

fn get_bincode_config() -> impl bincode::config::Config {
    bincode::config::standard()
        .with_little_endian()
        .with_fixed_int_encoding()
}

pub trait Syscall {
    /// Please randomly generate a u64 to avoid conflicts with other forks.
    /// Even if you change an existing syscall's behavior or inputs, change the ID.
    const ID: u64;

    type Input: Encode + Decode<()>;
    type Output: Encode + Decode<()>;

    #[cfg(feature = "user_mode")]
    /// For user mode to serialize the syscall input
    fn encode_input(input: &Self::Input) -> [u64; 7] {
        let mut output = [Default::default(); 7];
        output[0] = Self::ID;
        bincode::encode_into_slice(input, output[1..].as_mut_bytes(), get_bincode_config())
            .unwrap();
        output
    }

    #[cfg(feature = "user_mode")]
    /// For user mode to deserialize the output returned by the kernel
    fn decode_output(output: &[u64; 7]) -> Self::Output {
        let output_bytes = output.as_bytes();
        bincode::decode_from_slice(output_bytes, get_bincode_config())
            .unwrap()
            .0
    }

    #[cfg(feature = "kernel")]
    /// For the kernel to deserialize the (untrusted) syscall input
    fn try_decode_input(input: &[u64; 6]) -> Result<Self::Input, bincode::error::DecodeError> {
        Ok(bincode::decode_from_slice(input.as_bytes(), get_bincode_config())?.0)
    }

    #[cfg(feature = "kernel")]
    /// For the kernel to serialize its syscall output
    fn encode_output(output: &Self::Output) -> [u64; 7] {
        let mut output_u64s = [Default::default(); 7];
        bincode::encode_into_slice(output, output_u64s.as_mut_bytes(), get_bincode_config())
            .unwrap();
        output_u64s
    }
}

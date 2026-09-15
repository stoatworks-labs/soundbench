//! Raw PortAudio bindings.
//!
//! Hand-written rather than generated: the surface this app uses is a few
//! dozen functions and six structs, and a hand-written declaration is one the
//! reader can check against `vendor/portaudio/include/*.h` line by line.
//! Everything here is `unsafe` and mirrors the C API exactly; the safe layer
//! is `soundbench-audio`.
//!
//! Two ABI details worth knowing:
//!
//! - PortAudio's `unsigned long` is a **32-bit** type on Windows and 64-bit
//!   elsewhere. `c_ulong` tracks that; never substitute `u64`.
//! - The enums are C `int`s. They are declared as `c_int` constants rather
//!   than Rust enums so that an unknown value from a newer PortAudio cannot
//!   produce undefined behaviour on the Rust side.

#![allow(non_camel_case_types, non_upper_case_globals, non_snake_case)]

use std::os::raw::{c_char, c_double, c_int, c_long, c_ulong, c_void};

pub type PaError = c_int;
pub type PaDeviceIndex = c_int;
pub type PaHostApiIndex = c_int;
pub type PaHostApiTypeId = c_int;
pub type PaTime = c_double;
pub type PaSampleFormat = c_ulong;
pub type PaStreamFlags = c_ulong;
pub type PaStreamCallbackFlags = c_ulong;
pub type PaStream = c_void;

// PaErrorCode
pub const paNoError: PaError = 0;
pub const paNotInitialized: PaError = -10000;
pub const paUnanticipatedHostError: PaError = -9999;
pub const paInvalidChannelCount: PaError = -9998;
pub const paInvalidSampleRate: PaError = -9997;
pub const paInvalidDevice: PaError = -9996;
pub const paInvalidFlag: PaError = -9995;
pub const paSampleFormatNotSupported: PaError = -9994;
pub const paBadIODeviceCombination: PaError = -9993;
pub const paInsufficientMemory: PaError = -9992;
pub const paBufferTooBig: PaError = -9991;
pub const paBufferTooSmall: PaError = -9990;
pub const paNullCallback: PaError = -9989;
pub const paBadStreamPtr: PaError = -9988;
pub const paTimedOut: PaError = -9987;
pub const paInternalError: PaError = -9986;
pub const paDeviceUnavailable: PaError = -9985;
pub const paIncompatibleHostApiSpecificStreamInfo: PaError = -9984;
pub const paStreamIsStopped: PaError = -9983;
pub const paStreamIsNotStopped: PaError = -9982;
pub const paInputOverflowed: PaError = -9981;
pub const paOutputUnderflowed: PaError = -9980;
pub const paHostApiNotFound: PaError = -9979;
pub const paInvalidHostApi: PaError = -9978;
pub const paCanNotReadFromACallbackStream: PaError = -9977;
pub const paCanNotWriteToACallbackStream: PaError = -9976;
pub const paCanNotReadFromAnOutputOnlyStream: PaError = -9975;
pub const paCanNotWriteToAnInputOnlyStream: PaError = -9974;
pub const paIncompatibleStreamHostApi: PaError = -9973;
pub const paBadBufferPtr: PaError = -9972;
pub const paCanNotInitializeRecursively: PaError = -9971;

// PaHostApiTypeId
pub const paInDevelopment: PaHostApiTypeId = 0;
pub const paDirectSound: PaHostApiTypeId = 1;
pub const paMME: PaHostApiTypeId = 2;
pub const paASIO: PaHostApiTypeId = 3;
pub const paSoundManager: PaHostApiTypeId = 4;
pub const paCoreAudio: PaHostApiTypeId = 5;
pub const paOSS: PaHostApiTypeId = 7;
pub const paALSA: PaHostApiTypeId = 8;
pub const paAL: PaHostApiTypeId = 9;
pub const paBeOS: PaHostApiTypeId = 10;
pub const paWDMKS: PaHostApiTypeId = 11;
pub const paJACK: PaHostApiTypeId = 12;
pub const paWASAPI: PaHostApiTypeId = 13;
pub const paAudioScienceHPI: PaHostApiTypeId = 14;
pub const paAudioIO: PaHostApiTypeId = 15;
pub const paPulseAudio: PaHostApiTypeId = 16;
pub const paSndio: PaHostApiTypeId = 17;

pub const paNoDevice: PaDeviceIndex = -1;
pub const paUseHostApiSpecificDeviceSpecification: PaDeviceIndex = -2;

// PaSampleFormat
pub const paFloat32: PaSampleFormat = 0x0000_0001;
pub const paInt32: PaSampleFormat = 0x0000_0002;
pub const paInt24: PaSampleFormat = 0x0000_0004;
pub const paInt16: PaSampleFormat = 0x0000_0008;
pub const paInt8: PaSampleFormat = 0x0000_0010;
pub const paUInt8: PaSampleFormat = 0x0000_0020;
pub const paCustomFormat: PaSampleFormat = 0x0001_0000;
pub const paNonInterleaved: PaSampleFormat = 0x8000_0000;

pub const paFormatIsSupported: PaError = 0;
pub const paFramesPerBufferUnspecified: c_ulong = 0;

// PaStreamFlags
pub const paNoFlag: PaStreamFlags = 0;
pub const paClipOff: PaStreamFlags = 0x0000_0001;
pub const paDitherOff: PaStreamFlags = 0x0000_0002;
pub const paNeverDropInput: PaStreamFlags = 0x0000_0004;
pub const paPrimeOutputBuffersUsingStreamCallback: PaStreamFlags = 0x0000_0008;

// PaStreamCallbackFlags
pub const paInputUnderflow: PaStreamCallbackFlags = 0x0000_0001;
pub const paInputOverflow: PaStreamCallbackFlags = 0x0000_0002;
pub const paOutputUnderflow: PaStreamCallbackFlags = 0x0000_0004;
pub const paOutputOverflow: PaStreamCallbackFlags = 0x0000_0008;
pub const paPrimingOutput: PaStreamCallbackFlags = 0x0000_0010;

// PaStreamCallbackResult
pub const paContinue: c_int = 0;
pub const paComplete: c_int = 1;
pub const paAbort: c_int = 2;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaVersionInfo {
    pub versionMajor: c_int,
    pub versionMinor: c_int,
    pub versionSubMinor: c_int,
    pub versionControlRevision: *const c_char,
    pub versionText: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaHostApiInfo {
    pub structVersion: c_int,
    pub type_: PaHostApiTypeId,
    pub name: *const c_char,
    pub deviceCount: c_int,
    pub defaultInputDevice: PaDeviceIndex,
    pub defaultOutputDevice: PaDeviceIndex,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaHostErrorInfo {
    pub hostApiType: PaHostApiTypeId,
    pub errorCode: c_long,
    pub errorText: *const c_char,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaDeviceInfo {
    pub structVersion: c_int,
    pub name: *const c_char,
    pub hostApi: PaHostApiIndex,
    pub maxInputChannels: c_int,
    pub maxOutputChannels: c_int,
    pub defaultLowInputLatency: PaTime,
    pub defaultLowOutputLatency: PaTime,
    pub defaultHighInputLatency: PaTime,
    pub defaultHighOutputLatency: PaTime,
    pub defaultSampleRate: c_double,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaStreamParameters {
    pub device: PaDeviceIndex,
    pub channelCount: c_int,
    pub sampleFormat: PaSampleFormat,
    pub suggestedLatency: PaTime,
    pub hostApiSpecificStreamInfo: *mut c_void,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaStreamCallbackTimeInfo {
    pub inputBufferAdcTime: PaTime,
    pub currentTime: PaTime,
    pub outputBufferDacTime: PaTime,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PaStreamInfo {
    pub structVersion: c_int,
    pub inputLatency: PaTime,
    pub outputLatency: PaTime,
    pub sampleRate: c_double,
}

pub type PaStreamCallback = unsafe extern "C" fn(
    input: *const c_void,
    output: *mut c_void,
    frameCount: c_ulong,
    timeInfo: *const PaStreamCallbackTimeInfo,
    statusFlags: PaStreamCallbackFlags,
    userData: *mut c_void,
) -> c_int;

pub type PaStreamFinishedCallback = unsafe extern "C" fn(userData: *mut c_void);

extern "C" {
    pub fn Pa_GetVersion() -> c_int;
    pub fn Pa_GetVersionInfo() -> *const PaVersionInfo;
    pub fn Pa_GetErrorText(errorCode: PaError) -> *const c_char;
    pub fn Pa_Initialize() -> PaError;
    pub fn Pa_Terminate() -> PaError;
    pub fn Pa_GetHostApiCount() -> PaHostApiIndex;
    pub fn Pa_GetDefaultHostApi() -> PaHostApiIndex;
    pub fn Pa_GetHostApiInfo(hostApi: PaHostApiIndex) -> *const PaHostApiInfo;
    pub fn Pa_HostApiTypeIdToHostApiIndex(type_: PaHostApiTypeId) -> PaHostApiIndex;
    pub fn Pa_HostApiDeviceIndexToDeviceIndex(
        hostApi: PaHostApiIndex,
        hostApiDeviceIndex: c_int,
    ) -> PaDeviceIndex;
    pub fn Pa_GetLastHostErrorInfo() -> *const PaHostErrorInfo;
    pub fn Pa_GetDeviceCount() -> PaDeviceIndex;
    pub fn Pa_GetDefaultInputDevice() -> PaDeviceIndex;
    pub fn Pa_GetDefaultOutputDevice() -> PaDeviceIndex;
    pub fn Pa_GetDeviceInfo(device: PaDeviceIndex) -> *const PaDeviceInfo;
    pub fn Pa_IsFormatSupported(
        inputParameters: *const PaStreamParameters,
        outputParameters: *const PaStreamParameters,
        sampleRate: c_double,
    ) -> PaError;
    pub fn Pa_OpenStream(
        stream: *mut *mut PaStream,
        inputParameters: *const PaStreamParameters,
        outputParameters: *const PaStreamParameters,
        sampleRate: c_double,
        framesPerBuffer: c_ulong,
        streamFlags: PaStreamFlags,
        streamCallback: Option<PaStreamCallback>,
        userData: *mut c_void,
    ) -> PaError;
    pub fn Pa_CloseStream(stream: *mut PaStream) -> PaError;
    pub fn Pa_SetStreamFinishedCallback(
        stream: *mut PaStream,
        streamFinishedCallback: Option<PaStreamFinishedCallback>,
    ) -> PaError;
    pub fn Pa_StartStream(stream: *mut PaStream) -> PaError;
    pub fn Pa_StopStream(stream: *mut PaStream) -> PaError;
    pub fn Pa_AbortStream(stream: *mut PaStream) -> PaError;
    pub fn Pa_IsStreamStopped(stream: *mut PaStream) -> PaError;
    pub fn Pa_IsStreamActive(stream: *mut PaStream) -> PaError;
    pub fn Pa_GetStreamInfo(stream: *mut PaStream) -> *const PaStreamInfo;
    pub fn Pa_GetStreamTime(stream: *mut PaStream) -> PaTime;
    pub fn Pa_GetStreamCpuLoad(stream: *mut PaStream) -> c_double;
    pub fn Pa_GetSampleSize(format: PaSampleFormat) -> PaError;
    pub fn Pa_Sleep(msec: c_long);
}

// ---------------------------------------------------------------- CoreAudio

#[cfg(target_os = "macos")]
pub mod mac {
    use super::*;

    pub const paMacCoreChangeDeviceParameters: c_ulong = 0x01;
    pub const paMacCoreFailIfConversionRequired: c_ulong = 0x02;
    pub const paMacCoreConversionQualityMin: c_ulong = 0x0100;
    pub const paMacCoreConversionQualityMedium: c_ulong = 0x0200;
    pub const paMacCoreConversionQualityLow: c_ulong = 0x0300;
    pub const paMacCoreConversionQualityHigh: c_ulong = 0x0400;
    pub const paMacCoreConversionQualityMax: c_ulong = 0x0000;
    pub const paMacCorePlayNice: c_ulong = 0x00;
    pub const paMacCorePro: c_ulong = 0x01;
    pub const paMacCoreMinimizeCPUButPlayNice: c_ulong = 0x0100;
    pub const paMacCoreMinimizeCPU: c_ulong = 0x0101;

    pub type AudioDeviceID = u32;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct PaMacCoreStreamInfo {
        pub size: c_ulong,
        pub hostApiType: PaHostApiTypeId,
        pub version: c_ulong,
        pub flags: c_ulong,
        pub channelMap: *const i32,
        pub channelMapSize: c_ulong,
    }

    extern "C" {
        pub fn PaMacCore_SetupStreamInfo(data: *mut PaMacCoreStreamInfo, flags: c_ulong);
        pub fn PaMacCore_SetupChannelMap(
            data: *mut PaMacCoreStreamInfo,
            channelMap: *const i32,
            channelMapSize: c_ulong,
        );
        pub fn PaMacCore_GetStreamInputDevice(s: *mut PaStream) -> AudioDeviceID;
        pub fn PaMacCore_GetStreamOutputDevice(s: *mut PaStream) -> AudioDeviceID;
        pub fn PaMacCore_GetChannelName(
            device: c_int,
            channelIndex: c_int,
            input: bool,
        ) -> *const c_char;
        pub fn PaMacCore_GetBufferSizeRange(
            device: PaDeviceIndex,
            minBufferSizeFrames: *mut c_long,
            maxBufferSizeFrames: *mut c_long,
        ) -> PaError;
    }
}

// ---------------------------------------------------------------- Windows

#[cfg(target_os = "windows")]
pub mod win {
    use super::*;

    // pa_win_wasapi.h — PaWasapiFlags
    pub const paWinWasapiExclusive: c_ulong = 1 << 0;
    pub const paWinWasapiRedirectHostProcessor: c_ulong = 1 << 1;
    pub const paWinWasapiUseChannelMask: c_ulong = 1 << 2;
    pub const paWinWasapiPolling: c_ulong = 1 << 3;
    pub const paWinWasapiThreadPriority: c_ulong = 1 << 4;
    pub const paWinWasapiExplicitSampleFormat: c_ulong = 1 << 5;
    pub const paWinWasapiAutoConvert: c_ulong = 1 << 6;
    pub const paWinWasapiPassthrough: c_ulong = 1 << 7;

    pub type PaWasapiThreadPriority = c_int;
    pub const eThreadPriorityNone: PaWasapiThreadPriority = 0;
    pub const eThreadPriorityAudio: PaWasapiThreadPriority = 1;
    pub const eThreadPriorityProAudio: PaWasapiThreadPriority = 6;

    pub type PaWasapiStreamCategory = c_int;
    pub const eAudioCategoryOther: PaWasapiStreamCategory = 0;

    pub type PaWasapiStreamOption = c_int;
    pub const eStreamOptionNone: PaWasapiStreamOption = 0;
    pub const eStreamOptionRaw: PaWasapiStreamOption = 1;
    pub const eStreamOptionMatchFormat: PaWasapiStreamOption = 2;

    pub type PaWinWaveFormatChannelMask = c_ulong;
    pub type PaWasapiHostProcessorCallback = Option<
        unsafe extern "C" fn(
            inputBuffer: *mut c_void,
            inputFrames: c_long,
            outputBuffer: *mut c_void,
            outputFrames: c_long,
            userData: *mut c_void,
        ),
    >;

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct PaWasapiStreamPassthrough {
        pub formatId: c_int,
        pub encodedSamplesPerSec: u32,
        pub encodedChannelCount: u32,
        pub averageBytesPerSec: u32,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct PaWasapiStreamInfo {
        pub size: c_ulong,
        pub hostApiType: PaHostApiTypeId,
        pub version: c_ulong,
        pub flags: c_ulong,
        pub channelMask: PaWinWaveFormatChannelMask,
        pub hostProcessorOutput: PaWasapiHostProcessorCallback,
        pub hostProcessorInput: PaWasapiHostProcessorCallback,
        pub threadPriority: PaWasapiThreadPriority,
        pub streamCategory: PaWasapiStreamCategory,
        pub streamOption: PaWasapiStreamOption,
        pub passthrough: PaWasapiStreamPassthrough,
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    pub struct PaWasapiJackDescription {
        pub channelMapping: c_ulong,
        pub color: c_ulong,
        pub connectionType: c_int,
        pub geoLocation: c_int,
        pub genLocation: c_int,
        pub portConnection: c_int,
        pub isConnected: c_int,
    }

    extern "C" {
        pub fn PaWasapi_GetFramesPerHostBuffer(
            pStream: *mut PaStream,
            pInput: *mut u32,
            pOutput: *mut u32,
        ) -> PaError;
        pub fn PaWasapi_IsLoopback(device: PaDeviceIndex) -> c_int;
        pub fn PaWasapi_GetDeviceRole(device: PaDeviceIndex) -> c_int;
        pub fn PaWasapi_GetJackCount(device: PaDeviceIndex, pJackCount: *mut c_int) -> PaError;
        pub fn PaWasapi_GetJackDescription(
            device: PaDeviceIndex,
            jackIndex: c_int,
            pJackDescription: *mut PaWasapiJackDescription,
        ) -> PaError;
        /// Fills a WAVEFORMATEX(TENSIBLE); the caller inspects the raw bytes.
        pub fn PaWasapi_GetDeviceDefaultFormat(
            pFormat: *mut c_void,
            formatSize: u32,
            device: PaDeviceIndex,
        ) -> c_int;
        pub fn PaWasapi_GetDeviceMixFormat(
            pFormat: *mut c_void,
            formatSize: u32,
            device: PaDeviceIndex,
        ) -> c_int;
    }

    #[cfg(feature = "asio")]
    pub mod asio {
        use super::*;

        pub const paAsioUseChannelSelectors: c_ulong = 0x01;

        #[repr(C)]
        #[derive(Debug, Clone, Copy)]
        pub struct PaAsioStreamInfo {
            pub size: c_ulong,
            pub hostApiType: PaHostApiTypeId,
            pub version: c_ulong,
            pub flags: c_ulong,
            pub channelSelectors: *mut c_int,
        }

        extern "C" {
            pub fn PaAsio_GetAvailableBufferSizes(
                device: PaDeviceIndex,
                minBufferSizeFrames: *mut c_long,
                maxBufferSizeFrames: *mut c_long,
                preferredBufferSizeFrames: *mut c_long,
                granularity: *mut c_long,
            ) -> PaError;
            pub fn PaAsio_ShowControlPanel(device: PaDeviceIndex, systemSpecific: *mut c_void) -> PaError;
            pub fn PaAsio_GetInputChannelName(
                device: PaDeviceIndex,
                channelIndex: c_int,
                channelName: *mut *const c_char,
            ) -> PaError;
            pub fn PaAsio_GetOutputChannelName(
                device: PaDeviceIndex,
                channelIndex: c_int,
                channelName: *mut *const c_char,
            ) -> PaError;
            pub fn PaAsio_SetStreamSampleRate(stream: *mut PaStream, sampleRate: c_double) -> PaError;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_readable() {
        // Pa_GetVersionInfo does not require Pa_Initialize.
        let info = unsafe { Pa_GetVersionInfo() };
        assert!(!info.is_null());
        let info = unsafe { *info };
        assert_eq!(info.versionMajor, 19);
        let text = unsafe { std::ffi::CStr::from_ptr(info.versionText) };
        assert!(text.to_string_lossy().contains("PortAudio"));
    }
}

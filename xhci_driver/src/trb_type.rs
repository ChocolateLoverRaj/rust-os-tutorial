use num_enum::{IntoPrimitive, TryFromPrimitive};

/// xHCI TRB Types as per Spec Section 6.4.6 Table 6-91 (page 469-471)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
pub enum XhciTrbType {
    /// Reserved
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : no
    Reserved = 0,

    /// Normal
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    Normal = 1,

    /// Setup Stage
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    SetupStage = 2,

    /// Data Stage
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    DataStage = 3,

    /// Status Stage
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    StatusStage = 4,

    /// Isoch
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    Isoch = 5,

    /// Link
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : yes
    Link = 6,

    /// Event Data
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    EventData = 7,

    /// Noop
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : no
    /// Transfer Ring : yes
    Noop = 8,

    /// Enable Slot Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    EnableSlotCmd = 9,

    /// Disable Slot Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    DisableSlotCmd = 10,

    /// Address Device Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    AddressDeviceCmd = 11,

    /// Configure Endpoint Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    ConfigureEndpointCmd = 12,

    /// Evaluate Context Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    EvaluateContextCmd = 13,

    /// Reset Endpoint Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    ResetEndpointCmd = 14,

    /// Stop Endpoint Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    StopEndpointCmd = 15,

    /// Set TR Dequeue Pointer Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    SetTrDequeuePtrCmd = 16,

    /// Reset Device Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    ResetDeviceCmd = 17,

    /// Force Event Command (Optional, used with virtualization only)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    ForceEventCmd = 18,

    /// Negotiate Bandwidth Command (Optional)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    NegotiateBandwidthCmd = 19,

    /// Set Latency Tolerance Value Command (Optional)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    SetLatencyToleranceValueCmd = 20,

    /// Get Port Bandwidth Command (Optional)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    GetPortBandwidthCmd = 21,

    /// Force Header Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    ForceHeaderCmd = 22,

    /// Noop Command
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    NoopCmd = 23,

    /// Get Extended Property Command (Optional)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    GetExtendedPropertyCmd = 24,

    /// Set Extended Property Command (Optional)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : yes
    /// Event Ring    : no
    /// Transfer Ring : no
    SetExtendedPropertyCmd = 25,

    /// Transfer Event
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    TransferEvent = 32,

    /// Command Completion Event
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    CmdCompletionEvent = 33,

    /// Port Status Change Event
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    PortStatusChangeEvent = 34,

    /// Bandwidth Request Event (Optional)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    BandwidthRequestEvent = 35,

    /// Doorbell Event (Optional, used with virtualization only)
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    DoorbellEvent = 36,

    /// Host Controller Event
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    HostControllerEvent = 37,

    /// Device Notification Event
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    DeviceNotificationEvent = 38,

    /// MFIndex Wrap Event
    ///
    /// Allowed TRB Types
    /// -----------------
    /// Command Ring  : no
    /// Event Ring    : yes
    /// Transfer Ring : no
    MfindexWrapEvent = 39,
}

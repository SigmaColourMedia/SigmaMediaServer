struct ReceiverReport {}

struct ReportBlock {
    ssrc: u32,
    fraction_lost: u16,
    cum_lost: u16,
}
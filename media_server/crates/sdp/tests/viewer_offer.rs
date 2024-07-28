mod viewer_offer {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use sdp::resolvers::{NegotiatedSession, SDPResolver};

    const VALID_SDP_STREAMER_OFFER: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";
    const EXPECTED_FINGERPRINT: &str = "sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";
    fn init_tests() -> (SDPResolver, NegotiatedSession) {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let socket_addr = SocketAddr::new(ip, 52000);
        let sdp_resolver = SDPResolver::new(EXPECTED_FINGERPRINT, socket_addr);

        let streamer_session = sdp_resolver
            .accept_stream_offer(VALID_SDP_STREAMER_OFFER)
            .expect("Should resolve streamer SDP offer");

        (sdp_resolver, streamer_session)
    }

    /** Valid viewer offer:
          - Has ICE credentials
          - Is properly formatted
          - Media codecs match streamer session codecs
    */
    #[test]
    fn resolves_valid_viewer_offer() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let valid_offer = format!("v=0\r\n\
        o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
        a=group:BUNDLE 0 1\r\n\
        a=ice-options:trickle\r\n\
        a=msid-semantic:WMS *\r\n\
        m=audio 9 UDP/TLS/RTP/SAVPF 109 9 0 8 101\r\n\
        c=IN IP4 0.0.0.0\r\n\
        a=recvonly\r\n\
        a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
        a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
        a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
        a=fmtp:109 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
        a=fmtp:101 0-15\r\n\
        a=ice-pwd:{ice_password}\r\n\
        a=ice-ufrag:{ice_username}\r\n\
        a=mid:0\r\n\
        a=rtcp-mux\r\n\
        a=rtpmap:109 opus/48000/2\r\n\
        a=rtpmap:9 G722/8000/1\r\n\
        a=rtpmap:0 PCMU/8000\r\n\
        a=rtpmap:8 PCMA/8000\r\n\
        a=rtpmap:101 telephone-event/8000/1\r\n\
        a=setup:actpass\r\n\
        a=ssrc:{audio_ssrc}\r\n\
        m=video 9 UDP/TLS/RTP/SAVPF 126 127 120 124 121 125 123 122 119\r\n\
        c=IN IP4 0.0.0.0\r\n\
        a=recvonly\r\n\
        a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
        a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
        a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
        a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
        a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
        a=fmtp:126 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
        a=fmtp:127 apt=126\r\n\
        a=fmtp:120 max-fs=12288;max-fr=60\r\n\
        a=fmtp:124 apt=120\r\n\
        a=fmtp:121 max-fs=12288;max-fr=60\r\n\
        a=fmtp:125 apt=121\r\n\
        a=fmtp:119 apt=122\r\n\
        a=ice-pwd:{ice_password}\r\n\
        a=ice-ufrag:{ice_username}\r\n\
        a=mid:1\r\n\
        a=rtcp-fb:126 nack\r\n\
        a=rtcp-fb:126 nack pli\r\n\
        a=rtcp-fb:126 ccm fir\r\n\
        a=rtcp-fb:126 goog-remb\r\n\
        a=rtcp-fb:126 transport-cc\r\n\
        a=rtcp-fb:120 nack\r\n\
        a=rtcp-fb:120 nack pli\r\n\
        a=rtcp-fb:120 ccm fir\r\n\
        a=rtcp-fb:120 goog-remb\r\n\
        a=rtcp-fb:120 transport-cc\r\n\
        a=rtcp-fb:121 nack\r\n\
        a=rtcp-fb:121 nack pli\r\n\
        a=rtcp-fb:121 ccm fir\r\n\
        a=rtcp-fb:121 goog-remb\r\n\
        a=rtcp-fb:121 transport-cc\r\n\
        a=rtcp-fb:123 nack\r\n\
        a=rtcp-fb:123 nack pli\r\n\
        a=rtcp-fb:123 ccm fir\r\n\
        a=rtcp-fb:123 goog-remb\r\n\
        a=rtcp-fb:123 transport-cc\r\n\
        a=rtcp-fb:122 nack\r\n\
        a=rtcp-fb:122 nack pli\r\n\
        a=rtcp-fb:122 ccm fir\r\n\
        a=rtcp-fb:122 goog-remb\r\n\
        a=rtcp-fb:122 transport-cc\r\n\
        a=rtcp-mux\r\n\
        a=rtcp-rsize\r\n\
        a=rtpmap:126 H264/90000\r\n\
        a=rtpmap:127 rtx/90000\r\n\
        a=rtpmap:120 VP8/90000\r\n\
        a=rtpmap:124 rtx/90000\r\n\
        a=rtpmap:121 VP9/90000\r\n\
        a=rtpmap:125 rtx/90000\r\n\
        a=rtpmap:123 ulpfec/90000\r\n\
        a=rtpmap:122 red/90000\r\n\
        a=rtpmap:119 rtx/90000\r\n\
        a=setup:actpass\r\n\
        a=ssrc:{video_ssrc}\r\n", ice_username = expected_username, ice_password = expected_password, audio_ssrc = expected_audio_ssrc, video_ssrc = expected_video_ssrc);

        let (sdp_resolver, negotiated_session) = init_tests();

        let result = sdp_resolver
            .accept_viewer_offer(&valid_offer, &negotiated_session)
            .expect("Should resolve offer");

        // Validate ICE credentials
        assert_eq!(result.ice_credentials.remote_username, expected_username);
        assert_eq!(result.ice_credentials.remote_password, expected_password);

        // Validate AudioSession
        assert_eq!(
            result.audio_session.codec,
            negotiated_session.audio_session.codec
        );
        assert_eq!(result.audio_session.remote_ssrc, expected_audio_ssrc);

        // Validate VideoSession
        assert_eq!(
            result.video_session.codec,
            negotiated_session.video_session.codec
        );
        assert_eq!(
            result.video_session.capabilities,
            negotiated_session.video_session.capabilities
        );
        assert_eq!(result.video_session.remote_ssrc, expected_video_ssrc);
    }
}

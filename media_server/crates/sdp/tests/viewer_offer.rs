mod viewer_offer {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use sdp2::{NegotiatedSession, SDPResolver};

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
        let expected_audio_codec_payload_number: usize = 106;
        let expected_video_codec_payload_number: usize = 126;

        let valid_offer = format!("v=0\r\n\
        o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
        s=-\r\n\
        t=0 0\r\n\
        a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
        a=setup:actpass\r\n\
        a=group:BUNDLE 0 1\r\n\
        a=ice-options:trickle\r\n\
        a=msid-semantic:WMS *\r\n\
        m=audio 9 UDP/TLS/RTP/SAVPF {audio_codec_number} 9 0 8 101\r\n\
        c=IN IP4 0.0.0.0\r\n\
        a=recvonly\r\n\
        a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
        a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
        a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
        a=fmtp:{audio_codec_number} maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
        a=fmtp:101 0-15\r\n\
        a=ice-pwd:{ice_password}\r\n\
        a=ice-ufrag:{ice_username}\r\n\
        a=mid:0\r\n\
        a=rtcp-mux\r\n\
        a=rtpmap:{audio_codec_number} opus/48000/2\r\n\
        a=rtpmap:9 G722/8000/1\r\n\
        a=rtpmap:0 PCMU/8000\r\n\
        a=rtpmap:8 PCMA/8000\r\n\
        a=rtpmap:101 telephone-event/8000/1\r\n\
        a=setup:actpass\r\n\
        a=ssrc:{audio_ssrc}\r\n\
        m=video 9 UDP/TLS/RTP/SAVPF {video_codec_number} 127 120 124 121 125 123 122 119\r\n\
        c=IN IP4 0.0.0.0\r\n\
        a=recvonly\r\n\
        a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
        a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
        a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
        a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
        a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
        a=fmtp:{video_codec_number} profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
        a=fmtp:127 apt={video_codec_number}\r\n\
        a=fmtp:120 max-fs=12288;max-fr=60\r\n\
        a=fmtp:124 apt=120\r\n\
        a=fmtp:121 max-fs=12288;max-fr=60\r\n\
        a=fmtp:125 apt=121\r\n\
        a=fmtp:119 apt=122\r\n\
        a=ice-pwd:{ice_password}\r\n\
        a=ice-ufrag:{ice_username}\r\n\
        a=mid:1\r\n\
        a=rtcp-fb:{video_codec_number} nack\r\n\
        a=rtcp-fb:{video_codec_number} nack pli\r\n\
        a=rtcp-fb:{video_codec_number} ccm fir\r\n\
        a=rtcp-fb:{video_codec_number} goog-remb\r\n\
        a=rtcp-fb:{video_codec_number} transport-cc\r\n\
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
        a=rtpmap:{video_codec_number} H264/90000\r\n\
        a=rtpmap:127 rtx/90000\r\n\
        a=rtpmap:120 VP8/90000\r\n\
        a=rtpmap:124 rtx/90000\r\n\
        a=rtpmap:121 VP9/90000\r\n\
        a=rtpmap:125 rtx/90000\r\n\
        a=rtpmap:123 ulpfec/90000\r\n\
        a=rtpmap:122 red/90000\r\n\
        a=rtpmap:119 rtx/90000\r\n\
        a=setup:actpass\r\n\
        a=ssrc:{video_ssrc}\r\n", ice_username = expected_username, ice_password = expected_password, audio_ssrc = expected_audio_ssrc, video_ssrc = expected_video_ssrc, audio_codec_number = expected_audio_codec_payload_number, video_codec_number = expected_video_codec_payload_number);

        let (sdp_resolver, streamer_session) = init_tests();

        let viewer_session = sdp_resolver
            .accept_viewer_offer(&valid_offer, &streamer_session)
            .expect("Should resolve offer");

        // Validate ICE credentials
        assert_eq!(
            viewer_session.ice_credentials.remote_username,
            expected_username
        );
        assert_eq!(
            viewer_session.ice_credentials.remote_password,
            expected_password
        );

        // Validate AudioSession
        assert_eq!(
            viewer_session.audio_session.codec,
            streamer_session.audio_session.codec
        );
        assert_eq!(
            viewer_session.audio_session.remote_ssrc,
            expected_audio_ssrc
        );
        assert_eq!(
            viewer_session.audio_session.payload_number,
            expected_audio_codec_payload_number
        );

        // Validate VideoSession
        assert_eq!(
            viewer_session.video_session.codec,
            streamer_session.video_session.codec
        );
        assert_eq!(
            viewer_session.video_session.capabilities,
            streamer_session.video_session.capabilities
        );
        assert_eq!(
            viewer_session.video_session.remote_ssrc,
            expected_video_ssrc
        );
        assert_eq!(
            viewer_session.video_session.payload_number,
            expected_video_codec_payload_number
        );

        // The SDP answer structure & order should remain deterministic
        let expected_answer = format!(
            "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:{ice_username}\r\n\
    a=ice-pwd:{ice_password}\r\n\
    a=ice-options:ice2\r\n\
    a=ice-lite\r\n\
    a=fingerprint:{fingerprint}\r\n\
    a=setup:passive\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF {audio_codec_number}\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:{audio_codec_number} opus/48000/2\r\n\
    a=ssrc:{audio_ssrc}\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF {video_codec_number}\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:{video_codec_number} h264/90000\r\n\
    a=ssrc:{video_ssrc}\r\n\
    a=fmtp:{video_codec_number} {video_fmtp}\r\n",
            ice_username = viewer_session.ice_credentials.host_username,
            ice_password = viewer_session.ice_credentials.host_password,
            fingerprint = EXPECTED_FINGERPRINT,
            audio_codec_number = expected_audio_codec_payload_number,
            video_codec_number = expected_video_codec_payload_number,
            audio_ssrc = viewer_session.audio_session.host_ssrc,
            video_ssrc = viewer_session.video_session.host_ssrc,
            video_fmtp = viewer_session
                .video_session
                .capabilities
                .into_iter()
                .collect::<Vec<_>>()
                .join(";") //todo Figure out a better way to compare FMTP
        );

        assert_eq!(String::from(viewer_session.sdp_answer), expected_answer);
    }

    #[test]
    fn rejects_offer_without_matching_video_codec() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 VP8/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_offer_without_matching_video_fmtp() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=0;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_offer_without_audio_codec() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 G722/8000/1\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_offer_with_invalid_media_direction() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=sendonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=sendonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_offer_with_missing_ssrc() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
            a=rtpmap:127 rtx/90000\r\n\
            a=rtpmap:120 VP8/90000\r\n\
            a=rtpmap:124 rtx/90000\r\n\
            a=rtpmap:121 VP9/90000\r\n\
            a=rtpmap:125 rtx/90000\r\n\
            a=rtpmap:123 ulpfec/90000\r\n\
            a=rtpmap:122 red/90000\r\n\
            a=rtpmap:119 rtx/90000\r\n\
            a=setup:actpass\r\n", ice_username = expected_username, ice_password = expected_password);

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_offer_with_missing_ice_credentials() {
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
            a=rtpmap:127 rtx/90000\r\n\
            a=rtpmap:120 VP8/90000\r\n\
            a=rtpmap:124 rtx/90000\r\n\
            a=rtpmap:121 VP9/90000\r\n\
            a=rtpmap:125 rtx/90000\r\n\
            a=rtpmap:123 ulpfec/90000\r\n\
            a=rtpmap:122 red/90000\r\n\
            a=rtpmap:119 rtx/90000\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{video_ssrc}\r\n", audio_ssrc = expected_audio_ssrc, video_ssrc = expected_video_ssrc);

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_non_demuxed_offer() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtcp-rsize\r\n\
            a=rtpmap:106 H264/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_non_bundled_offer() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=setup:actpass\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    #[test]
    fn rejects_invalid_dtls_role() {
        let expected_username = "aedfe975";
        let expected_password = "07393aecfec48f9ca7f41cc50d366ad9";
        let expected_audio_ssrc: u32 = 455694368;
        let expected_video_ssrc: u32 = 3804541430;

        let offer = format!("v=0\r\n\
            o=mozilla...THIS_IS_SDPARTA-99.0 7213999912078531628 0 IN IP4 0.0.0.0\r\n\
            s=-\r\n\
            t=0 0\r\n\
            a=fingerprint:sha-256 26:62:C5:CB:BF:68:B0:42:0E:DE:40:2B:30:B3:8F:38:04:CD:D4:9E:D3:EC:9D:D7:03:48:EC:9F:AA:92:9D:34\r\n\
            a=group:BUNDLE 0 1\r\n\
            a=ice-options:trickle\r\n\
            a=msid-semantic:WMS *\r\n\
            m=audio 9 UDP/TLS/RTP/SAVPF 111 9 0 8 101\r\n\
            a=setup:passive\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=recvonly\r\n\
            a=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\n\
            a=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=fmtp:111 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\n\
            a=fmtp:101 0-15\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:0\r\n\
            a=rtcp-mux\r\n\
            a=rtpmap:111 opus/48000/2\r\n\
            a=rtpmap:9 G722/8000/1\r\n\
            a=rtpmap:0 PCMU/8000\r\n\
            a=rtpmap:8 PCMA/8000\r\n\
            a=rtpmap:101 telephone-event/8000/1\r\n\
            a=setup:actpass\r\n\
            a=ssrc:{audio_ssrc}\r\n\
            m=video 9 UDP/TLS/RTP/SAVPF 106 127 120 124 121 125 123 122 119\r\n\
            c=IN IP4 0.0.0.0\r\n\
            a=setup:passive\r\n\
            a=recvonly\r\n\
            a=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\n\
            a=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\n\
            a=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\n\
            a=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\n\
            a=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\n\
            a=fmtp:106 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\n\
            a=fmtp:127 apt=106\r\n\
            a=fmtp:120 max-fs=12288;max-fr=60\r\n\
            a=fmtp:124 apt=120\r\n\
            a=fmtp:121 max-fs=12288;max-fr=60\r\n\
            a=fmtp:125 apt=121\r\n\
            a=fmtp:119 apt=122\r\n\
            a=ice-pwd:{ice_password}\r\n\
            a=ice-ufrag:{ice_username}\r\n\
            a=mid:1\r\n\
            a=rtcp-fb:106 nack\r\n\
            a=rtcp-fb:106 nack pli\r\n\
            a=rtcp-fb:106 ccm fir\r\n\
            a=rtcp-fb:106 goog-remb\r\n\
            a=rtcp-fb:106 transport-cc\r\n\
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
            a=rtpmap:106 H264/90000\r\n\
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

        let (sdp_resolver, streamer_session) = init_tests();

        sdp_resolver
            .accept_viewer_offer(&offer, &streamer_session)
            .expect_err("Should reject offer");
    }

    const EXAMPLE_BROWSER_OFFERS: [&str; 2] = ["v=0\r\no=mozilla...THIS_IS_SDPARTA-99.0 6639292399016986376 0 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\na=fingerprint:sha-256 20:E8:37:73:5C:0B:53:36:CE:27:11:31:CE:19:C0:99:9C:9B:A5:5E:68:CF:C0:5B:6F:6B:11:56:A2:E5:76:F0\r\na=group:BUNDLE 0 1\r\na=ice-options:trickle\r\na=msid-semantic:WMS *\r\nm=audio 9 UDP/TLS/RTP/SAVPF 109 9 0 8 101\r\nc=IN IP4 0.0.0.0\r\na=recvonly\r\na=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\na=extmap:2/recvonly urn:ietf:params:rtp-hdrext:csrc-audio-level\r\na=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\na=fmtp:109 maxplaybackrate=48000;stereo=1;useinbandfec=1\r\na=fmtp:101 0-15\r\na=ice-pwd:b0a8e5ab619c83b32932d8121a6246bd\r\na=ice-ufrag:d1427639\r\na=mid:0\r\na=rtcp-mux\r\na=rtpmap:109 opus/48000/2\r\na=rtpmap:9 G722/8000/1\r\na=rtpmap:0 PCMU/8000\r\na=rtpmap:8 PCMA/8000\r\na=rtpmap:101 telephone-event/8000/1\r\na=setup:actpass\r\na=ssrc:1943081398 cname:{2c8a6692-45f6-4f9f-8181-e3e0e0f3c160}\r\nm=video 9 UDP/TLS/RTP/SAVPF 126 127 120 124 121 125 123 122 119\r\nc=IN IP4 0.0.0.0\r\na=recvonly\r\na=extmap:3 urn:ietf:params:rtp-hdrext:sdes:mid\r\na=extmap:4 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\na=extmap:5 urn:ietf:params:rtp-hdrext:toffset\r\na=extmap:6/recvonly http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\na=extmap:7 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\na=fmtp:126 profile-level-id=42e01f;level-asymmetry-allowed=1;packetization-mode=1\r\na=fmtp:127 apt=126\r\na=fmtp:120 max-fs=12288;max-fr=60\r\na=fmtp:124 apt=120\r\na=fmtp:121 max-fs=12288;max-fr=60\r\na=fmtp:125 apt=121\r\na=fmtp:119 apt=122\r\na=ice-pwd:b0a8e5ab619c83b32932d8121a6246bd\r\na=ice-ufrag:d1427639\r\na=mid:1\r\na=rtcp-fb:126 nack\r\na=rtcp-fb:126 nack pli\r\na=rtcp-fb:126 ccm fir\r\na=rtcp-fb:126 goog-remb\r\na=rtcp-fb:126 transport-cc\r\na=rtcp-fb:120 nack\r\na=rtcp-fb:120 nack pli\r\na=rtcp-fb:120 ccm fir\r\na=rtcp-fb:120 goog-remb\r\na=rtcp-fb:120 transport-cc\r\na=rtcp-fb:121 nack\r\na=rtcp-fb:121 nack pli\r\na=rtcp-fb:121 ccm fir\r\na=rtcp-fb:121 goog-remb\r\na=rtcp-fb:121 transport-cc\r\na=rtcp-fb:123 nack\r\na=rtcp-fb:123 nack pli\r\na=rtcp-fb:123 ccm fir\r\na=rtcp-fb:123 goog-remb\r\na=rtcp-fb:123 transport-cc\r\na=rtcp-fb:122 nack\r\na=rtcp-fb:122 nack pli\r\na=rtcp-fb:122 ccm fir\r\na=rtcp-fb:122 goog-remb\r\na=rtcp-fb:122 transport-cc\r\na=rtcp-mux\r\na=rtcp-rsize\r\na=rtpmap:126 H264/90000\r\na=rtpmap:127 rtx/90000\r\na=rtpmap:120 VP8/90000\r\na=rtpmap:124 rtx/90000\r\na=rtpmap:121 VP9/90000\r\na=rtpmap:125 rtx/90000\r\na=rtpmap:123 ulpfec/90000\r\na=rtpmap:122 red/90000\r\na=rtpmap:119 rtx/90000\r\na=setup:actpass\r\na=ssrc:3796383410 cname:{2c8a6692-45f6-4f9f-8181-e3e0e0f3c160}\r\n",          "v=0\r\no=- 9071898453171296627 2 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=extmap-allow-mixed\r\na=msid-semantic: WMS\r\nm=audio 9 UDP/TLS/RTP/SAVPF 111 63 9 0 8 13 110 126\r\nc=IN IP4 0.0.0.0\r\na=rtcp:9 IN IP4 0.0.0.0\r\na=ice-ufrag:QD6R\r\na=ice-pwd:PWMuciTb4wn9UaevikffgsO1\r\na=ice-options:trickle\r\na=fingerprint:sha-256 2B:1E:1D:EC:AE:DC:10:1F:F4:59:4C:BA:BD:7F:57:89:AB:54:91:D8:2C:E4:1A:24:50:EF:37:0B:4A:FD:B3:9A\r\na=setup:actpass\r\na=mid:0\r\na=extmap:1 urn:ietf:params:rtp-hdrext:ssrc-audio-level\r\na=extmap:2 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\na=extmap:3 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\na=extmap:4 urn:ietf:params:rtp-hdrext:sdes:mid\r\na=recvonly\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=rtcp-fb:111 transport-cc\r\na=fmtp:111 minptime=10;useinbandfec=1\r\na=rtpmap:63 red/48000/2\r\na=fmtp:63 111/111\r\na=rtpmap:9 G722/8000\r\na=rtpmap:0 PCMU/8000\r\na=rtpmap:8 PCMA/8000\r\na=rtpmap:13 CN/8000\r\na=rtpmap:110 telephone-event/48000\r\na=rtpmap:126 telephone-event/8000\r\nm=video 9 UDP/TLS/RTP/SAVPF 96 97 98 99 100 101 35 36 37 38 102 103 104 105 106 107 108 109 127 125 39 40 41 42 43 44 45 46 47 48 112 113 114 49\r\nc=IN IP4 0.0.0.0\r\na=rtcp:9 IN IP4 0.0.0.0\r\na=ice-ufrag:QD6R\r\na=ice-pwd:PWMuciTb4wn9UaevikffgsO1\r\na=ice-options:trickle\r\na=fingerprint:sha-256 2B:1E:1D:EC:AE:DC:10:1F:F4:59:4C:BA:BD:7F:57:89:AB:54:91:D8:2C:E4:1A:24:50:EF:37:0B:4A:FD:B3:9A\r\na=setup:actpass\r\na=mid:1\r\na=extmap:14 urn:ietf:params:rtp-hdrext:toffset\r\na=extmap:2 http://www.webrtc.org/experiments/rtp-hdrext/abs-send-time\r\na=extmap:13 urn:3gpp:video-orientation\r\na=extmap:3 http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01\r\na=extmap:5 http://www.webrtc.org/experiments/rtp-hdrext/playout-delay\r\na=extmap:6 http://www.webrtc.org/experiments/rtp-hdrext/video-content-type\r\na=extmap:7 http://www.webrtc.org/experiments/rtp-hdrext/video-timing\r\na=extmap:8 http://www.webrtc.org/experiments/rtp-hdrext/color-space\r\na=extmap:4 urn:ietf:params:rtp-hdrext:sdes:mid\r\na=extmap:10 urn:ietf:params:rtp-hdrext:sdes:rtp-stream-id\r\na=extmap:11 urn:ietf:params:rtp-hdrext:sdes:repaired-rtp-stream-id\r\na=recvonly\r\na=rtcp-mux\r\na=rtcp-rsize\r\na=rtpmap:96 VP8/90000\r\na=rtcp-fb:96 goog-remb\r\na=rtcp-fb:96 transport-cc\r\na=rtcp-fb:96 ccm fir\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtpmap:97 rtx/90000\r\na=fmtp:97 apt=96\r\na=rtpmap:98 VP9/90000\r\na=rtcp-fb:98 goog-remb\r\na=rtcp-fb:98 transport-cc\r\na=rtcp-fb:98 ccm fir\r\na=rtcp-fb:98 nack\r\na=rtcp-fb:98 nack pli\r\na=fmtp:98 profile-id=0\r\na=rtpmap:99 rtx/90000\r\na=fmtp:99 apt=98\r\na=rtpmap:100 VP9/90000\r\na=rtcp-fb:100 goog-remb\r\na=rtcp-fb:100 transport-cc\r\na=rtcp-fb:100 ccm fir\r\na=rtcp-fb:100 nack\r\na=rtcp-fb:100 nack pli\r\na=fmtp:100 profile-id=2\r\na=rtpmap:101 rtx/90000\r\na=fmtp:101 apt=100\r\na=rtpmap:35 VP9/90000\r\na=rtcp-fb:35 goog-remb\r\na=rtcp-fb:35 transport-cc\r\na=rtcp-fb:35 ccm fir\r\na=rtcp-fb:35 nack\r\na=rtcp-fb:35 nack pli\r\na=fmtp:35 profile-id=1\r\na=rtpmap:36 rtx/90000\r\na=fmtp:36 apt=35\r\na=rtpmap:37 VP9/90000\r\na=rtcp-fb:37 goog-remb\r\na=rtcp-fb:37 transport-cc\r\na=rtcp-fb:37 ccm fir\r\na=rtcp-fb:37 nack\r\na=rtcp-fb:37 nack pli\r\na=fmtp:37 profile-id=3\r\na=rtpmap:38 rtx/90000\r\na=fmtp:38 apt=37\r\na=rtpmap:102 H264/90000\r\na=rtcp-fb:102 goog-remb\r\na=rtcp-fb:102 transport-cc\r\na=rtcp-fb:102 ccm fir\r\na=rtcp-fb:102 nack\r\na=rtcp-fb:102 nack pli\r\na=fmtp:102 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42001f\r\na=rtpmap:103 rtx/90000\r\na=fmtp:103 apt=102\r\na=rtpmap:104 H264/90000\r\na=rtcp-fb:104 goog-remb\r\na=rtcp-fb:104 transport-cc\r\na=rtcp-fb:104 ccm fir\r\na=rtcp-fb:104 nack\r\na=rtcp-fb:104 nack pli\r\na=fmtp:104 level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42001f\r\na=rtpmap:105 rtx/90000\r\na=fmtp:105 apt=104\r\na=rtpmap:106 H264/90000\r\na=rtcp-fb:106 goog-remb\r\na=rtcp-fb:106 transport-cc\r\na=rtcp-fb:106 ccm fir\r\na=rtcp-fb:106 nack\r\na=rtcp-fb:106 nack pli\r\na=fmtp:106 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=42e01f\r\na=rtpmap:107 rtx/90000\r\na=fmtp:107 apt=106\r\na=rtpmap:108 H264/90000\r\na=rtcp-fb:108 goog-remb\r\na=rtcp-fb:108 transport-cc\r\na=rtcp-fb:108 ccm fir\r\na=rtcp-fb:108 nack\r\na=rtcp-fb:108 nack pli\r\na=fmtp:108 level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=42e01f\r\na=rtpmap:109 rtx/90000\r\na=fmtp:109 apt=108\r\na=rtpmap:127 H264/90000\r\na=rtcp-fb:127 goog-remb\r\na=rtcp-fb:127 transport-cc\r\na=rtcp-fb:127 ccm fir\r\na=rtcp-fb:127 nack\r\na=rtcp-fb:127 nack pli\r\na=fmtp:127 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=4d001f\r\na=rtpmap:125 rtx/90000\r\na=fmtp:125 apt=127\r\na=rtpmap:39 H264/90000\r\na=rtcp-fb:39 goog-remb\r\na=rtcp-fb:39 transport-cc\r\na=rtcp-fb:39 ccm fir\r\na=rtcp-fb:39 nack\r\na=rtcp-fb:39 nack pli\r\na=fmtp:39 level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=4d001f\r\na=rtpmap:40 rtx/90000\r\na=fmtp:40 apt=39\r\na=rtpmap:41 H264/90000\r\na=rtcp-fb:41 goog-remb\r\na=rtcp-fb:41 transport-cc\r\na=rtcp-fb:41 ccm fir\r\na=rtcp-fb:41 nack\r\na=rtcp-fb:41 nack pli\r\na=fmtp:41 level-asymmetry-allowed=1;packetization-mode=1;profile-level-id=f4001f\r\na=rtpmap:42 rtx/90000\r\na=fmtp:42 apt=41\r\na=rtpmap:43 H264/90000\r\na=rtcp-fb:43 goog-remb\r\na=rtcp-fb:43 transport-cc\r\na=rtcp-fb:43 ccm fir\r\na=rtcp-fb:43 nack\r\na=rtcp-fb:43 nack pli\r\na=fmtp:43 level-asymmetry-allowed=1;packetization-mode=0;profile-level-id=f4001f\r\na=rtpmap:44 rtx/90000\r\na=fmtp:44 apt=43\r\na=rtpmap:45 AV1/90000\r\na=rtcp-fb:45 goog-remb\r\na=rtcp-fb:45 transport-cc\r\na=rtcp-fb:45 ccm fir\r\na=rtcp-fb:45 nack\r\na=rtcp-fb:45 nack pli\r\na=fmtp:45 level-idx=5;profile=0;tier=0\r\na=rtpmap:46 rtx/90000\r\na=fmtp:46 apt=45\r\na=rtpmap:47 AV1/90000\r\na=rtcp-fb:47 goog-remb\r\na=rtcp-fb:47 transport-cc\r\na=rtcp-fb:47 ccm fir\r\na=rtcp-fb:47 nack\r\na=rtcp-fb:47 nack pli\r\na=fmtp:47 level-idx=5;profile=1;tier=0\r\na=rtpmap:48 rtx/90000\r\na=fmtp:48 apt=47\r\na=rtpmap:112 red/90000\r\na=rtpmap:113 rtx/90000\r\na=fmtp:113 apt=112\r\na=rtpmap:114 ulpfec/90000\r\na=rtpmap:49 flexfec-03/90000\r\na=rtcp-fb:49 goog-remb\r\na=rtcp-fb:49 transport-cc\r\na=fmtp:49 repair-window=10000000\r\n"];

    #[test]
    fn resolves_browser_offers() {
        let (sdp_resolver, streamer_session) = init_tests();

        EXAMPLE_BROWSER_OFFERS.iter().for_each(|&offer| {
            sdp_resolver
                .accept_viewer_offer(offer, &streamer_session)
                .expect("Should resolve offer");
        });
    }
}

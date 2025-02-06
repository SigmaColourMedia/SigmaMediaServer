mod streamer_offer {
    use std::collections::HashSet;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use sdp::{AudioCodec, SDPResolver, VideoCodec};

    const EXPECTED_FINGERPRINT: &str = "sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B";

    fn init_sdp_resolver() -> SDPResolver {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let socket_addr = SocketAddr::new(ip, 52000);
        SDPResolver::new(EXPECTED_FINGERPRINT, socket_addr)
    }

    const VALID_SDP_OFFER: &str = "v=0\r\no=rtc 3767197920 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\na=group:BUNDLE 0 1\r\na=group:LS 0 1\r\na=msid-semantic:WMS *\r\na=setup:actpass\r\na=ice-ufrag:E2Fr\r\na=ice-pwd:OpQzg1PAwUdeOB244chlgd\r\na=ice-options:trickle\r\na=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\nm=audio 4557 UDP/TLS/RTP/SAVPF 111\r\nc=IN IP4 192.168.0.198\r\na=mid:0\r\na=sendonly\r\na=ssrc:1349455989 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455989 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-audio\r\na=rtcp-mux\r\na=rtpmap:111 opus/48000/2\r\na=fmtp:111 minptime=10;maxaveragebitrate=96000;stereo=1;sprop-stereo=1;useinbandfec=1\r\na=candidate:1 1 UDP 2015363327 192.168.0.198 4557 typ host\r\na=candidate:2 1 UDP 2015363583 fe80::6c3d:5b42:1532:2f9a 10007 typ host\r\na=end-of-candidates\r\nm=video 4557 UDP/TLS/RTP/SAVPF 96\r\nc=IN IP4 192.168.0.198\r\na=mid:1\r\na=sendonly\r\na=ssrc:1349455990 cname:0X2NGAsK9XcmnsuZ\r\na=ssrc:1349455990 msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=msid:qUVEoh7TF9nLCrk4 qUVEoh7TF9nLCrk4-video\r\na=rtcp-mux\r\na=rtpmap:96 H264/90000\r\na=rtcp-fb:96 nack\r\na=rtcp-fb:96 nack pli\r\na=rtcp-fb:96 goog-remb\r\na=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

    #[test]
    fn resolves_valid_sdp() {
        let sdp_resolver = init_sdp_resolver();

        let negotiated_session = sdp_resolver
            .accept_stream_offer(VALID_SDP_OFFER)
            .expect("Should resolve offer");

        // remote ICE ice_credentials should match
        assert_eq!(negotiated_session.ice_credentials.remote_username, "E2Fr");
        assert_eq!(
            negotiated_session.ice_credentials.remote_password,
            "OpQzg1PAwUdeOB244chlgd"
        );

        // AudioSession should match offer audio media
        assert_eq!(negotiated_session.audio_session.codec, AudioCodec::Opus);
        assert_eq!(negotiated_session.audio_session.payload_number, 111);
        assert_eq!(
            negotiated_session.audio_session.remote_ssrc,
            Some(1349455989)
        );

        // VideoSession should match offer video media
        assert_eq!(negotiated_session.video_session.codec, VideoCodec::H264);
        assert_eq!(negotiated_session.video_session.payload_number, 96);
        assert_eq!(
            negotiated_session.video_session.remote_ssrc,
            Some(1349455990)
        );
        assert_eq!(
            negotiated_session.video_session.capabilities,
            HashSet::from([
                "profile-level-id=42e01f".to_string(),
                "packetization-mode=1".to_string(),
                "level-asymmetry-allowed=1".to_string()
            ])
        );

        let actual_answer = String::from(negotiated_session.sdp_answer);

        // The SDP answer structure & order should remain deterministic
        let expected_answer = format!(
            "v=0\r\n\
    o=SMID 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=SMID\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:{ice_username}\r\n\
    a=ice-pwd:{ice_password}\r\n\
    a=ice-options:ice2\r\n\
    a=ice-lite\r\n\
    a=fingerprint:{fingerprint}\r\n\
    a=setup:passive\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=recvonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:{audio_ssrc} cname:SMID\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=recvonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=ssrc:{video_ssrc} cname:SMID\r\n\
    a=fmtp:96 {video_fmtp}\r\n\
    a=rtcp-fb:96 nack\r\n\
    a=rtcp-fb:96 nack pli\r\n",
            ice_username = negotiated_session.ice_credentials.host_username,
            ice_password = negotiated_session.ice_credentials.host_password,
            fingerprint = EXPECTED_FINGERPRINT,
            audio_ssrc = negotiated_session.audio_session.host_ssrc,
            video_ssrc = negotiated_session.video_session.host_ssrc,
            video_fmtp = negotiated_session
                .video_session
                .capabilities
                .into_iter()
                .collect::<Vec<_>>()
                .join(";") //todo Figure out a better way to compare FMTP
        );

        assert_eq!(
            expected_answer, actual_answer,
            "SDP answer should match excepted answer"
        );
    }

    #[test]
    fn rejects_sdp_with_unsupported_video_codecs() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn resolves_sdp_with_multiple_video_codecs() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2 cname:my-cname\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1 cname:my-cname\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        let result = sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect("Should resolve SDP");

        assert_eq!(result.video_session.codec, VideoCodec::H264)
    }

    #[test]
    fn rejects_invalid_direction_offer() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=recvonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=recvonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn rejects_offer_with_missing_ssrc() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        let negotiated_session = sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect("Should resolve SDP");

        assert_eq!(negotiated_session.video_session.remote_ssrc, None);
        assert_eq!(negotiated_session.audio_session.remote_ssrc, None);
    }

    #[test]
    fn rejects_non_demuxed_sdp() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn rejects_non_bundled_media() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn rejects_invalid_dtls_role() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:passive\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=mid:0\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=mid:1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn rejects_invalid_formatted_sdp() {
        // Swap session-media-level attributes order
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    t=0 0\r\n\
    s=smid\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn rejects_offer_with_missing_video_fmtp() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-ufrag:username\r\n\
    a=ice-pwd:password\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }

    #[test]
    fn rejects_media_with_missing_credentials() {
        let sdp_offer = "v=0\r\n\
    o=smid 3767197920 0 IN IP4 127.0.0.1\r\n\
    s=smid\r\n\
    t=0 0\r\n\
    a=group:BUNDLE 0 1\r\n\
    a=ice-options:ice2\r\n\
    a=fingerprint:sha-256 EF:53:C9:F2:E0:A0:4F:1D:5E:99:4C:20:B8:D7:DE:21:3B:58:15:C4:E5:88:87:46:65:27:F7:3B:C6:DC:EF:3B\r\n\
    a=setup:actpass\r\n\
    m=audio 52000 UDP/TLS/RTP/SAVPF 111\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:0\r\n\
    a=candidate:1 1 UDP 2015363327 127.0.0.1 52000 typ host\r\n\
    a=end-of-candidates\r\n\
    a=rtpmap:111 opus/48000/2\r\n\
    a=ssrc:2\r\n\
    m=video 52000 UDP/TLS/RTP/SAVPF 96 97\r\n\
    c=IN IP4 127.0.0.1\r\n\
    a=sendonly\r\n\
    a=rtcp-mux\r\n\
    a=mid:1\r\n\
    a=rtpmap:96 h264/90000\r\n\
    a=rtpmap:97 v9/90000\r\n\
    a=ssrc:1\r\n\
    a=fmtp:96 profile-level-id=42e01f;packetization-mode=1;level-asymmetry-allowed=1\r\n";

        let sdp_resolver = init_sdp_resolver();
        sdp_resolver
            .accept_stream_offer(sdp_offer)
            .expect_err("Should reject SDP");
    }
}

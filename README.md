# SigmaMediaServer

## Description
My own spin of a video streaming service. The app allows a compliant client (_streamer_) to establish a WebRTC connection with the remote host and stream a **H.264**-encoded video. The video may then be 
shared with and watched by other remote WebRTC clients (_viewers_).

Some notable features:
- Establish WebRTC connections with _streamer_ clients using the _WHIP_ protocol
- Establish WebRTC connections with _viewer_ clients using the _WHEP_ protocol
- Forward _streamer_ video packets down to respective _viewer_ clients.
- Generate _jpeg_ thumbnails for streamed videos.
- [Server Sent Events](https://datatracker.ietf.org/doc/html/rfc8895#name-server-push-server-sent-eve) API providing information about current _streamer_ lobbies

***This project is very much a learning exercise. I intentionally avoided using ready solutions (even though this could probably make the project easier to maintain). There are few dependencies which I don't
think I could do without, and so some will just have to stay.***
  
## How to use this
First thing you need is to get the server up and running. You can follow the **Building & development** guide for that.

The app runs two servers: one on a UDP socket (handles video-packets), the other on a TCP socket. The TCP socket has a barebones HTTP server with following resources available:
- POST `/whip` - a WHIP protocol endpoint
- POST `/whep` - a WHEP protocol endpoint
- GET `/rooms` - get available _rooms_ as a JSON. A _room_ is basically a virtual lobby, stored in-app-memory, containing information such as the room's `id` and the `viewer_count`.
You'll need the `id` for interacting with the `WHEP` endpoint.
- GET `/notifications` - a SSE endpoint. Streams _rooms_ JSON every so often.

### _Streamer_ client

You'll need a WHIP-compliant client software. My personal choice is the [OBS software](https://obsproject.com/). Assuming it's yours too, the setup is following:
- Open `Settings -> Stream`
- Choose `WHIP` as a "Service"
- Insert the WHIP endpoint as a "Server" value. The WHIP endpoint corresponds to an HTTP address constructed from `TCP_ADDRESS` and `TCP_PORT` environment variables, and a `/whip` resource pathname.
- Insert `WHIP_TOKEN` environment variable value as a "Bearer Token" value.

![image](https://github.com/user-attachments/assets/fbc474ee-9d58-4a68-b17d-63a47e8a7b5b)

For any other clients just follow the WHIP endpoint specification.

Once a connection is established, the _streamer_ should acquire it's own _room_. To get available rooms, check the `/rooms` endpoint. 

### _Viewer_ client

First thing you need is the _room_'s id you're interested in joining. Use the `/rooms` endpoint to check what's available.

A connection requires a WHEP-compliant software. My personal choice is, well, the browser. Checkout [SigmaPlayer](https://github.com/SigmaColourMedia/SigmaPlayer) for an exemplary implementation. The `WHEP` endpoint expects a query parameter
`target_id`, which specifies a concrete _room_'s id. If your client does not support _streamer_'s codecs, the `WHEP` request will fail.

Once you establish a WebRTC connection with the remote server, you should be forwarded the _streamer_'s video packets.


## Building & development

You'll need following system dependencies:
- `openssh`
- `openssh-devel`
- `libsrtp`

The `openssh` is used for establishing a DTLS connection with the remote peer. The `libsrtp` is used for encrypting and decrypting RTP/SRTP packets.  If you're still having trouble compiling the app, file an issue (or if you feel adventurous
you may try and follow the compiler errors to figure out what dependencies are missing)

You'll need the following environment variables exported to your shell:
- `TCP_ADDRESS`
- `TCP_PORT`
- `UDP_ADDRESS`
- `UDP_PORT`
- `WHIP_TOKEN` - A secret token used to authorize clients using the `WHIP` route. This token is required for all clients that wish to become _streamers_. This token is shared for all _streamer_ clients.
- `FRONTEND_URL` - A URL of the frontend web-app that interacts with the HTTP server. Ideally this is the URL of a web-app that works as a streaming platform, allowing clients to become _viewers_. Used for CORS.
- `STORAGE_DIR` - System directory where temporary _streamer_ video's thumbnail images will be generated and saved to. The app should have write permissions for that directory.
- `CERTS_DIR` - System directory where TLS key & certificate are stored. The files should be named `key.pem` and `cert.pem`. There is no good reason for this being so opinionated. These are used for establishing a DTLS connection with remote peers.
 You may use following command to generate needed files: `openssl req -newkey rsa:2048 -new -nodes -x509 -days 3650 -keyout key.pem -out cert.pem`

The `STORAGE_DIR` and `CERTS_DIR` directories need to actually exist in your file system - the app won't create them for you.

You may then compile and run the app using `cargo run`. If everything goes right, you should see the TCP & UDP server addresses printed out to your shell.
The build a production release, use `cargo build --release`.

## How does it work?

The app implements following protocols:
- [WHIP](https://datatracker.ietf.org/doc/draft-ietf-wish-whip/)
- [WHEP](https://datatracker.ietf.org/doc/draft-murillo-whep/)
- [ICE-lite](https://datatracker.ietf.org/doc/html/rfc8445)
- [STUN](https://datatracker.ietf.org/doc/html/rfc5389)
  
Some of these are not followed "by the book", but rather by whatever felt absolutely necessary for my purposes. I do hope to make it, eventually, 100% RFC-compliant though.


The app binds to two network sockets, one UDP, one TCP. Both of these sockets are expected to be exposed to the public network. A HTTP server runs on a TCP socket. This server is responsible for opening client connections and sharing _room_ data. The UDP server handles video & STUN packets. The high-level overview looks as follows:
![image](https://github.com/user-attachments/assets/aabb54c7-44c5-4521-82c3-dac25e64fcb6)

When a remote client wishes to become a _streamer_, it sends a WHIP request containing an SDP offer. The SDP contains information like available video & audio codecs, ICE credentials, fingerprint etc.. If the server accepts the offer (e.g. the offer has all the valid codecs, demuxing audio and video), an SDP answer is sent back, indicating agreed upon codecs, ICE credentials, fingerprint and an ICE candidate. The ICE candidate is the UDP server.
![image](https://github.com/user-attachments/assets/71a86a19-84ea-4c53-baaa-22eb1f32ba79)

An ICE protocol is now used to establish a UDP connection with the host. The host acts as an ICE-lite server, whilst the remote client acts as full ICE. The remote client will send STUN packets using a [short-term credentials mechanism](https://datatracker.ietf.org/doc/html/rfc5389#autoid-22). The credentials are a combination of host and remote ICE-credentials exchanged just at the WHIP endpoint step. At some point, a remote client must nominate its peer. It does so by attaching a special attribute to the STUN message. When that happens, a remote client is officially recognized as a _streamer_ by the host app. The STUN binding requests will continue for the lifetime of the connection, serving as "life-checks".
![image](https://github.com/user-attachments/assets/b119b3e8-917e-49d7-a275-b7885bd63685)

The next step is to establish a DTLS connection. A remote client will attempt to establish a DTLS connection with the host (as specified by the SDP offer/answer). A DTLS is encrypted using provided TLS keys. The `fingerprint` value exchanged in the SDP process presents a TLS certificate of both peers. The remote peer knows it's really the remote host when the remote host proves its certificate matches the one exchanged in the SDP. Since the SDP exchange happens at HTTPS level (production-wise), we trust the `fingerprint` values. Once a DTLS connection is established, we get the secret key that is used to encrypt messages sent through this channel. Note that neither the host nor remote will communicate through the DTLS. All data (STUN and video packets) are still sent through UDP. DTLS is used only to derive cryptographic keys.

The host app will now accept video packets. The remote peer sends video packets to the host UDP socket. The packets are encapsulated in RTP protocol. The RTP consists of a header with information like _payload number, ssrc, sequence number_ and payload - raw codec data. The RTP itself is encrypted using the keys derived from the DTLS connection, hence the remote peer actually sends SRTP packets. The host app is capable of decoding these packets into RTP. 
![image](https://github.com/user-attachments/assets/1ccf3442-cdb7-4572-a502-e902bdffbb41)

In a very similar manner, a host-remote connection is established with _viewer_ clients, with few exceptions. The _streamer_ client is linked with a _room_, i.e. a virtual lobby representing it's media stream. The _viewer_ client is also linked with a _room_, but the client has no ownership over that data. Once the SRTP packet coming from a recognized _streamer_ client is decoded, two things happen. One - video thumbnail data is generated (if enough data is available), two -  the video packet is forwarded to all _viewer_ clients linked to the _streamer's room_. For each _viewer_ client, the RTP packet is encrypted into the SRTP packet using the associated DTLS keys. The packet is then sent through UDP to the remote peer.
![image](https://github.com/user-attachments/assets/a9a9dced-57ad-4a5d-a5b2-a9a74e851ba3)





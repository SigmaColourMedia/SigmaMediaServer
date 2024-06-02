import {customElement, property, state} from "lit/decorators.js";
import {html, LitElement} from "lit";
import LoaderStyles from "./styles/loader.css" assert {type: "css"};
import StreamStyles from "./styles/stream.css" assert {type: "css"};

enum StreamState {
    LoadingStreamData,
    EstablishingRTC,
    LoadingVideo,
    Active,
    NotFound,
    UnknownError
}

function getLoadingLabel(state: StreamState) {
    switch (state) {
        case StreamState.LoadingStreamData:
            return "Acquiring stream data..."
        case StreamState.EstablishingRTC:
            return "Establishing RTC connection..."
        case StreamState.LoadingVideo:
            return "Loading Video..."
        default:
            return ""
    }
}

@customElement('s-stream')
export class Stream extends LitElement {
    static styles = [LoaderStyles, StreamStyles];

    @property() streamID: string
    @state() streamSate: StreamState = StreamState.LoadingStreamData
    @state() connection: RTCPeerConnection | null = null
    @state() video: HTMLVideoElement | null = null

    async establishRTC(sdp: string) {
        try {
            const rtcConnection = new RTCPeerConnection();
            rtcConnection.ontrack = ({track, streams}) => {
                this.streamSate = StreamState.LoadingVideo

                const video = document.createElement("video")
                this.video = video

                video.controls = true;
                video.className = "video"
                video.width = 1240;

                video.srcObject = streams[0]
                video.onloadeddata = () => {
                    this.streamSate = StreamState.Active
                }
            }

            rtcConnection.onconnectionstatechange = () => {
                switch (rtcConnection.connectionState) {
                    case "closed": {
                        this.streamSate = StreamState.UnknownError
                        break
                    }
                    case "failed": {
                        this.streamSate = StreamState.UnknownError
                        break
                    }
                }
            }
            await rtcConnection.setRemoteDescription({sdp, type: "offer"})

            rtcConnection.addTransceiver('audio', {direction: 'recvonly'})
            rtcConnection.addTransceiver('video', {direction: 'recvonly'})
            const answer = await rtcConnection.createAnswer()
            await rtcConnection.setLocalDescription(answer)
        } catch {
            this.streamSate = StreamState.UnknownError
        }

    }

    connectedCallback() {
        super.connectedCallback();
        fetch(`whep?target_id=${this.streamID}`).then(res => {
            if (!res.ok) {
                this.streamSate = StreamState.NotFound
                return
            }
            res.text().then((sdp) => this.establishRTC.call(this, sdp))
        })
    }

    render() {
        switch (this.streamSate) {
            case StreamState.LoadingStreamData: {
                return html`
                    <div class="loader-wrapper">
                        <svg fill="none" height="48" class="loader" viewBox="0 0 48 48" width="48"
                             xmlns="http://www.w3.org/2000/svg">
                            <path d="M48 24C48 28.7468 46.5924 33.3869 43.9553 37.3337C41.3181 41.2805 37.5698 44.3566 33.1844 46.1731C28.799 47.9896 23.9734 48.4649 19.3178 47.5388C14.6623 46.6128 10.3859 44.327 7.02944 40.9706C3.67298 37.6141 1.3872 33.3377 0.461153 28.6822C-0.464892 24.0266 0.0103881 19.201 1.82689 14.8156C3.64339 10.4302 6.71953 6.68188 10.6663 4.04473C14.6131 1.40758 19.2532 -5.66044e-08 24 0V4.8C20.2026 4.8 16.4905 5.92606 13.3331 8.03578C10.1756 10.1455 7.71472 13.1441 6.26151 16.6525C4.80831 20.1608 4.42809 24.0213 5.16892 27.7457C5.90976 31.4702 7.73838 34.8913 10.4235 37.5765C13.1087 40.2616 16.5298 42.0902 20.2543 42.8311C23.9787 43.5719 27.8392 43.1917 31.3475 41.7385C34.8559 40.2853 37.8545 37.8244 39.9642 34.667C42.0739 31.5095 43.2 27.7974 43.2 24H48Z"
                                  fill="#8347E0"/>
                        </svg>
                        <h2>${getLoadingLabel(this.streamSate)}</h2>
                    </div>
                `
            }
            case StreamState.EstablishingRTC: {
                return html`
                    <div class="loader-wrapper">
                        <svg fill="none" height="48" class="loader" viewBox="0 0 48 48" width="48"
                             xmlns="http://www.w3.org/2000/svg">
                            <path d="M48 24C48 28.7468 46.5924 33.3869 43.9553 37.3337C41.3181 41.2805 37.5698 44.3566 33.1844 46.1731C28.799 47.9896 23.9734 48.4649 19.3178 47.5388C14.6623 46.6128 10.3859 44.327 7.02944 40.9706C3.67298 37.6141 1.3872 33.3377 0.461153 28.6822C-0.464892 24.0266 0.0103881 19.201 1.82689 14.8156C3.64339 10.4302 6.71953 6.68188 10.6663 4.04473C14.6131 1.40758 19.2532 -5.66044e-08 24 0V4.8C20.2026 4.8 16.4905 5.92606 13.3331 8.03578C10.1756 10.1455 7.71472 13.1441 6.26151 16.6525C4.80831 20.1608 4.42809 24.0213 5.16892 27.7457C5.90976 31.4702 7.73838 34.8913 10.4235 37.5765C13.1087 40.2616 16.5298 42.0902 20.2543 42.8311C23.9787 43.5719 27.8392 43.1917 31.3475 41.7385C34.8559 40.2853 37.8545 37.8244 39.9642 34.667C42.0739 31.5095 43.2 27.7974 43.2 24H48Z"
                                  fill="#8347E0"/>
                        </svg>
                        <h2>${getLoadingLabel(this.streamSate)}</h2>
                    </div>
                `
            }
            case StreamState.LoadingVideo: {
                return html`
                    <div class="loader-wrapper">
                        <svg fill="none" height="48" class="loader" viewBox="0 0 48 48" width="48"
                             xmlns="http://www.w3.org/2000/svg">
                            <path d="M48 24C48 28.7468 46.5924 33.3869 43.9553 37.3337C41.3181 41.2805 37.5698 44.3566 33.1844 46.1731C28.799 47.9896 23.9734 48.4649 19.3178 47.5388C14.6623 46.6128 10.3859 44.327 7.02944 40.9706C3.67298 37.6141 1.3872 33.3377 0.461153 28.6822C-0.464892 24.0266 0.0103881 19.201 1.82689 14.8156C3.64339 10.4302 6.71953 6.68188 10.6663 4.04473C14.6131 1.40758 19.2532 -5.66044e-08 24 0V4.8C20.2026 4.8 16.4905 5.92606 13.3331 8.03578C10.1756 10.1455 7.71472 13.1441 6.26151 16.6525C4.80831 20.1608 4.42809 24.0213 5.16892 27.7457C5.90976 31.4702 7.73838 34.8913 10.4235 37.5765C13.1087 40.2616 16.5298 42.0902 20.2543 42.8311C23.9787 43.5719 27.8392 43.1917 31.3475 41.7385C34.8559 40.2853 37.8545 37.8244 39.9642 34.667C42.0739 31.5095 43.2 27.7974 43.2 24H48Z"
                                  fill="#8347E0"/>
                        </svg>
                        <h2>${getLoadingLabel(this.streamSate)}</h2>
                    </div>
                `
            }
            case StreamState.NotFound: {
                return html`
                    <div class="loader-wrapper">
                        <h2>404 Stream not found :(</h2>
                    </div>
                `
            }
            case StreamState.UnknownError: {
                return html`
                    <div class="loader-wrapper">
                        <h2>500 Stream error :(</h2>
                    </div>
                `
            }
            case StreamState.Active: {
                this.renderRoot.querySelector("#wrapper")?.appendChild(this.video!)

                return html`
                    <div id="wrapper" class="loader-wrapper">
                        <div class="video-wrapper">
                            ${this.video}
                        </div>
                    </div>
                `
            }

        }
    }
}


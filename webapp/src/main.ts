import {customElement, property} from 'lit/decorators.js';
import {html, LitElement} from "lit";
import MainStyles from "./styles/main.css" assert {type: "css"}
import "./lobby"
import "./stream"

@customElement('s-main')
export class Main extends LitElement {
    static styles = MainStyles;

    @property() search = window.location.search


    render() {
        const searchTarget = new URLSearchParams(window.location.search).get("watch")
        if (searchTarget) return html`
            <s-stream streamID=${searchTarget}></s-stream>`

        return html`
            <s-rooms></s-rooms>`;
    }
}


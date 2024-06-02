import {customElement} from 'lit/decorators.js';
import {html, LitElement} from "lit";
import MainStyles from "./styles/main.css" assert {type: "css"}

@customElement('s-main')
export class Main extends LitElement {
    static styles = MainStyles;

    render() {
        return html`
            <main>
                <header>hell2o</header>
                <div>hello</div>
            </main>`;
    }
}
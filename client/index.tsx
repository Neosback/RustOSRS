import React, { Suspense, lazy } from "react";
import ReactDOM from "react-dom/client";
import { BrowserRouter } from "react-router-dom";

import "./index.css";
import reportWebVitals from "./reportWebVitals";
import { registerServiceWorker } from "./serviceWorkerRegistration";

const Page = lazy(() => import("./game/GamePage"));

const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
root.render(
    // <React.StrictMode>
    <BrowserRouter>
        <Suspense fallback={<div className="page-loading">Loading…</div>}>
            <Page />
        </Suspense>
    </BrowserRouter>,
    // </React.StrictMode>,
);

// If you want to start measuring performance in your app, pass a function
// to log results (for example: reportWebVitals(console.log))
// or send to an analytics endpoint. Learn more: https://bit.ly/CRA-vitals
reportWebVitals();

registerServiceWorker();

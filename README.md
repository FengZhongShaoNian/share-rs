# share-rs

This is a LAN file transfer (sharing) tool that supports file sharding upload and breakpoint resumption.
The desktop UI is built with Rust and [GPUI Kit](https://gpui-kit.com), the HTTP backend uses Actix-web + Sea-ORM, and the web frontend is developed with React.

Main interface：
![](screenshot/app-ui.png)

Server information interface: (You can scan the QR code to open the sharing page through the mobile phone browser)
![](screenshot/app-server-info.png)

Share Page:
![](screenshot/web-share.png)

Upload page:
![](screenshot/web-upload.png)
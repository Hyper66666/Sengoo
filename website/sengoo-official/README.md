# Sengoo Official Website

Standalone static website for `www.sengoo.top`.

## Files

- `index.html` - single-page language homepage.
- `assets/css/styles.css` - responsive visual system.
- `assets/js/main.js` - mobile navigation and subtle card interaction.

## Deploy Target

Recommended server path:

```text
/var/www/sengoo-official
```

Recommended Nginx server block:

```nginx
server {
    listen 80;
    listen [::]:80;
    server_name www.sengoo.top sengoo.top;

    root /var/www/sengoo-official;
    index index.html;

    location / {
        try_files $uri $uri/ /index.html;
    }
}
```

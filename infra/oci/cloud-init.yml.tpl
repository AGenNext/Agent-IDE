#cloud-config
package_update: true
packages:
  - podman
  - caddy
  - curl
  - git

write_files:
  - path: /etc/${app_name}.env
    permissions: "0600"
    content: |
      NODE_ENV=production
      PORT=${port}

  - path: /etc/caddy/Caddyfile
    content: |
      ${domain} {
          reverse_proxy localhost:${port}
      }

  - path: /etc/containers/systemd/${app_name}.container
    content: |
      [Unit]
      Description=${app_name}
      After=network-online.target

      [Container]
      Image=${image}
      EnvironmentFile=/etc/${app_name}.env
      PublishPort=127.0.0.1:${port}:${port}
      Volume=/var/lib/${app_name}:/data
      AutoUpdate=registry
      %{ if ghcr_token != "" }
      Secret=ghcr-token,type=env,target=REGISTRY_AUTH_TOKEN
      %{ endif }

      [Service]
      Restart=on-failure

      [Install]
      WantedBy=multi-user.target

runcmd:
  # OCI iptables (Ubuntu blocks 80/443 by default)
  - iptables  -I INPUT 6 -p tcp --dport 80  -j ACCEPT
  - iptables  -I INPUT 6 -p tcp --dport 443 -j ACCEPT
  - netfilter-persistent save || true

  # GHCR auth
  %{ if ghcr_token != "" }
  - echo "${ghcr_token}" | podman login ghcr.io -u "${ghcr_user}" --password-stdin
  %{ endif }

  # Pull image
  - podman pull ${image}

  # Create data dir
  - mkdir -p /var/lib/${app_name}

  # Start app + enable auto-update
  - systemctl daemon-reload
  - systemctl enable --now ${app_name}
  - systemctl enable --now podman-auto-update.timer

  # Start Caddy
  - systemctl enable --now caddy

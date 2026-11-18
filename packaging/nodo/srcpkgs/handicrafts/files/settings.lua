-- The shop's settings on monokulo-nodo. Each is named after the HANDICRAFTS_* variable it fills
-- (files/launcher maps them); src/config.rs and src/mail.rs read them.
local function email(value)
  return nodo.matches(value, "^[^@ ]+@[^@ ]+\\.[^@ ]+$"), "must be an email address"
end

local function web_address(url)
  return nodo.matches(url, "^https?://[A-Za-z0-9.\\[\\]:-]+$"),
    "must be http:// or https:// with no path or trailing slash"
end

local function count(name, label, help, default)
  return {
    name = name,
    type = "int",
    label = label,
    help = help,
    default = default,
    min = 1,
    max = 100000,
  }
end

return {
  api_version = 1,
  settings = {
    -- The shop
    {
      name = "public_url",
      type = "url",
      label = "Public address",
      help = "The shop's address, used for pay links in emails and invoices and for the return "
        .. "from connecting Monokulo. Give the site this address in Hosting.",
      required = true,
      default = "https://www.rachelshandicrafts.com",
      validate = web_address,
    },
    {
      name = "admin_user",
      type = "string",
      label = "Admin user",
      help = "The user name for /admin, where orders are kept track of.",
      required = true,
      default = "rachel",
      validate = function(user)
        return nodo.matches(user, "^[^: ]{1,64}$"), "must be up to 64 characters, no spaces or colons"
      end,
    },
    {
      name = "admin_password_hash",
      type = "secret",
      label = "Admin password hash",
      help = "The argon2 hash of /admin's password. Make one over SSH with "
        .. "/opt/nodo/apps/handicrafts/libexec/handicrafts hash-password, typing the password "
        .. "on its input. Until it is set, /admin stays closed.",
      validate = function(hash)
        return nodo.matches(hash, "^\\$argon2(id|i|d)\\$[^ ]+$"), "must be an argon2 hash ($argon2id$...)"
      end,
    },
    {
      name = "admin_email",
      type = "string",
      label = "Notifications to",
      help = "Where order and payment notifications go.",
      validate = email,
    },

    -- Email
    {
      name = "mail",
      type = "enum",
      label = "Email",
      help = "off: emails wait in the outbox. smtp or tuta: they are sent. log: they are written "
        .. "to the service's log instead, for testing.",
      choices = { "off", "smtp", "tuta", "log" },
      default = "off",
    },
    {
      name = "mail_from",
      type = "string",
      label = "Send from",
      help = "Tuta: empty sends from the account's own address, or give one of its aliases. "
        .. "SMTP: empty uses the SMTP user.",
      validate = email,
    },
    {
      name = "mail_from_name",
      type = "string",
      label = "Sender name",
      help = "SMTP only; Tuta sends with the name set in Tuta.",
      default = "Rachel's Handicrafts",
    },
    {
      name = "tuta_user",
      type = "string",
      label = "Tuta account",
      help = "The Tuta account's address. Accounts with two-factor sign-in cannot be used.",
      validate = email,
    },
    {
      name = "tuta_password",
      type = "secret",
      label = "Tuta password",
    },
    {
      name = "smtp_host",
      type = "string",
      label = "SMTP server",
      help = "Such as smtp.example.com.",
      validate = function(host)
        return nodo.matches(host, "^[A-Za-z0-9.-]+$"), "must be a host name"
      end,
    },
    {
      name = "smtp_port",
      type = "int",
      label = "SMTP port",
      help = "Empty: 465 for tls, 587 for starttls, 25 for none.",
      min = 1,
      max = 65535,
    },
    {
      name = "smtp_security",
      type = "enum",
      label = "SMTP security",
      choices = { "tls", "starttls", "none" },
      default = "tls",
    },
    {
      name = "smtp_user",
      type = "string",
      label = "SMTP user",
      help = "Empty if the server needs no login.",
    },
    {
      name = "smtp_password",
      type = "secret",
      label = "SMTP password",
    },

    -- Bank transfer, shown on the pay page
    {
      name = "payid",
      type = "string",
      label = "PayID",
    },
    {
      name = "bank_account_name",
      type = "string",
      label = "Account name",
    },
    {
      name = "bsb",
      type = "string",
      label = "BSB",
      validate = function(bsb)
        return nodo.matches(bsb, "^[0-9]{3}-?[0-9]{3}$"), "must be six digits, such as 062-000"
      end,
    },
    {
      name = "account_number",
      type = "string",
      label = "Account number",
      validate = function(number)
        return nodo.matches(number, "^[0-9 ]{4,12}$"), "must be digits"
      end,
    },

    -- Monero, through Monokulo
    {
      name = "monokulo_url",
      type = "url",
      label = "Monokulo address",
      help = "Then connect the store at /admin/monokulo, which fetches its keys itself.",
      default = "https://www.monokulo.com",
      validate = web_address,
    },
    {
      name = "monokulo_public_key",
      type = "string",
      label = "Monokulo public key",
      help = "Only for keys pasted by hand instead of connecting at /admin/monokulo: then all "
        .. "three keys are needed.",
      validate = function(key)
        return nodo.matches(key, "^pk_[A-Za-z0-9_-]+$"), "must start with pk_"
      end,
    },
    {
      name = "monokulo_secret_key",
      type = "secret",
      label = "Monokulo secret key",
      validate = function(key)
        return nodo.matches(key, "^sk_[A-Za-z0-9_-]+$"), "must start with sk_"
      end,
    },
    {
      name = "monokulo_webhook_secret",
      type = "secret",
      label = "Monokulo webhook secret",
    },

    -- Spam limits on orders
    count("limit_ip_10min", "Orders per address, 10 minutes",
      "Orders one client address may place in 10 minutes.", 3),
    count("limit_ip_day", "Orders per address, day", "Orders one client address may place in a day.", 10),
    count("limit_email_day", "Orders per email, day", "Orders one email address may place in a day.", 3),
    count("limit_shop_day", "Orders per day", "Orders the whole shop takes in a day.", 40),
  },
}

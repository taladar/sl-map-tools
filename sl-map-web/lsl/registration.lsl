// sl-map-web — in-world registration object.
//
// Touch the prim this script lives in to register (or re-register, which
// doubles as a password reset) on the sl-map-web service. The script POSTs
// the avatar's UUID, legacy name and firstname.lastname username to
// /api/auth/register using a pre-shared bearer token, then privately chats
// the returned one-time set-password URL back to the toucher.
//
// Distribute the prim no-modify so recipients cannot read these constants.
// SERVER_URL must match the value of SL_MAP_WEB_PUBLIC_BASE_URL on the
// server, and BEARER_TOKEN must match SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN.
// SERVER_URL must use https:// — the server's session cookie defaults to
// Secure, so plain http will not work end-to-end.

string SERVER_URL   = "https://maps.example.org";
string BEARER_TOKEN = "REPLACE-ME-WITH-THE-REGISTRATION-BEARER-TOKEN";

// In-flight HTTP requests as a flat list of [request_id, avatar_key, ...]
// so we can route http_response events back to the right toucher and
// handle concurrent clicks.
list pending;

default
{
    state_entry()
    {
        llSetText("Touch to register with sl-map-web", <1.0, 1.0, 1.0>, 1.0);
    }

    touch_start(integer count)
    {
        integer i;
        for (i = 0; i < count; ++i)
        {
            key    avatar      = llDetectedKey(i);
            string legacy_name = llDetectedName(i);
            string username    = llGetUsername(avatar);
            if (username == "")
            {
                llRegionSayTo(avatar, 0,
                    "Could not look up your username (are you in-region?). Please try again.");
            }
            else
            {
                string body = llList2Json(JSON_OBJECT, [
                    "agent_key",   (string)avatar,
                    "legacy_name", legacy_name,
                    "username",    username
                ]);
                key req = llHTTPRequest(SERVER_URL + "/api/auth/register", [
                    HTTP_METHOD,        "POST",
                    HTTP_MIMETYPE,      "application/json",
                    HTTP_CUSTOM_HEADER, "Authorization", "Bearer " + BEARER_TOKEN,
                    HTTP_VERIFY_CERT,   TRUE
                ], body);
                pending += [req, avatar];
                llRegionSayTo(avatar, 0, "Contacting sl-map-web…");
            }
        }
    }

    http_response(key request_id, integer status, list metadata, string body)
    {
        integer idx = llListFindList(pending, [request_id]);
        if (idx == -1) return;
        key avatar = (key)llList2String(pending, idx + 1);
        pending = llDeleteSubList(pending, idx, idx + 1);

        if (status != 200)
        {
            llRegionSayTo(avatar, 0,
                "Registration failed (HTTP " + (string)status +
                "). Please notify the object owner.");
            return;
        }
        string url = llJsonGetValue(body, ["set_password_url"]);
        if (url == JSON_INVALID || url == "")
        {
            llRegionSayTo(avatar, 0,
                "Registration succeeded but the server did not return a link.");
            return;
        }
        string expires = llJsonGetValue(body, ["expires_at"]);
        string suffix = "";
        if (expires != JSON_INVALID && expires != "")
        {
            suffix = " (expires at " + expires + ")";
        }
        llRegionSayTo(avatar, 0,
            "Open this one-time link in your browser to set your password" +
            suffix + ":");
        llRegionSayTo(avatar, 0, url);
    }
}

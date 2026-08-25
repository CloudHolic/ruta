-- Dumps every string chunk passed to `load` while the suite runs, for the parse corpus.
-- Framing is length-prefixed because chunks contain newlines and any delimiter we could pick.

local real = load
local out = assert(io.open(assert(os.getenv("RUTA_CAPTURE")), "ab"))

_ENV.load = function (chunk, ...)
    if type(chunk) == "string" then
        out:write(("%d\n"):format(#chunk), chunk)
        out:flush()
    end
    return real(chunk, ...)
end

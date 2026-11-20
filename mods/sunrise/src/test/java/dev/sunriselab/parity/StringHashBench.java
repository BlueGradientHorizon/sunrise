package dev.sunriselab.parity;

/**
 * HotSpot's String.hashCode per call on fresh strings, for tools/string_hash_kit: MIX LEN ROUNDS [stepped|check], or with
 * the mode prefixed `create-` (new String(utf8 bytes, UTF_8) then hashCode, per string) or `lookup-` (HashMap.get of 64
 * String keys whose hash is already cached, per lookup).
 * Each batch makes the row's strings anew (untimed: new String(char[], off, len), compact Latin-1 when every unit
 * fits), then times one hashCode call on each, so that no hash is cached. The strings are the same as
 * tools/string_hash_kit/bench.rs makes.
 */
public final class StringHashBench {
    static volatile int sink;
    static String[] fresh;

    public static void main(String[] a) throws Exception {
        String mix = a[0];
        int len = Integer.parseInt(a[1]);
        int count = Math.max(64, Math.min(4096, 32768 / Math.max(len, 1)));
        char[] pool = new char[count * len];
        long state = 1;
        for (int i = 0; i < count; i++) {
            int n = 0;
            while (n < len) {
                state = state * 6364136223846793005L + 1442695040888963407L;
                int r = (int) (state >>> 33);
                int at = i * len + n;
                switch (mix) {
                    case "ascii" -> { pool[at] = (char) (0x20 + r % 95); n++; }
                    case "latin1" -> { pool[at] = (char) (0x20 + r % 224); n++; }
                    case "bmp" -> { pool[at] = (char) (0x100 + r % (0xD800 - 0x100)); n++; }
                    case "surrogates" -> {
                        if (n + 1 < len) {
                            pool[at] = (char) (0xD800 + r % 0x400);
                            pool[at + 1] = (char) (0xDC00 + (r >>> 10) % 0x400);
                            n += 2;
                        } else { pool[at] = (char) (0x100 + r % (0xD800 - 0x100)); n++; }
                    }
                    default -> throw new IllegalArgumentException("unknown mix " + mix);
                }
            }
        }
        fresh = new String[count];
        String mode = a.length > 3 ? a[3] : "stepped";
        boolean check = mode.endsWith("check");
        if (mode.startsWith("create-")) {
            byte[][] utf8 = new byte[count][];
            for (int i = 0; i < count; i++) utf8[i] = new String(pool, i * len, len).getBytes(java.nio.charset.StandardCharsets.UTF_8);
            run(a, check, () -> createAll(utf8), () -> {}, count);
            return;
        }
        if (mode.startsWith("lookup-")) {
            java.util.HashMap<String, Integer> map = new java.util.HashMap<>();
            String[] queries = new String[64];
            for (int i = 0; i < 64; i++) {
                map.put(new String(pool, i * len, len), i);
                queries[i] = new String(pool, i * len, len);
                queries[i].hashCode();
            }
            run(a, check, () -> lookupAll(map, queries), () -> {}, 64 * 16);
            return;
        }
        run(a, check, () -> hashAll(), () -> make(pool, len), count);
    }

    /** One timed batch per round: `prepare` untimed, then `work` timed, `per` calls in it. */
    static void run(String[] a, boolean check, java.util.function.IntSupplier work, Runnable prepare, int per)
            throws Exception {
        if (check) {
            prepare.run();
            System.out.println(work.getAsInt());
            return;
        }
        int rounds = Integer.parseInt(a[2]);
        boolean stepped = a.length > 3 && a[3].endsWith("stepped");
        java.io.BufferedReader in = new java.io.BufferedReader(new java.io.InputStreamReader(System.in));
        for (int round = 0; round < rounds; round++) {
            if (stepped && in.readLine() == null) return;
            for (int w = 0; w < 2000; w++) { prepare.run(); sink = work.getAsInt(); }
            double[] t = new double[400];
            for (int b = 0; b < 400; b++) {
                prepare.run();
                long s = System.nanoTime();
                sink = work.getAsInt();
                t[b] = (System.nanoTime() - s) / (double) per;
            }
            java.util.Arrays.sort(t);
            System.out.printf("hotspot median %.4f ns%n", t[200]);
            System.out.flush();
        }
    }

    static int createAll(byte[][] utf8) {
        int s = 0;
        for (byte[] b : utf8) s += new String(b, java.nio.charset.StandardCharsets.UTF_8).hashCode();
        return s;
    }

    static int lookupAll(java.util.HashMap<String, Integer> map, String[] queries) {
        int s = 0;
        for (int pass = 0; pass < 16; pass++) for (String q : queries) s += map.get(q);
        return s;
    }

    static void make(char[] pool, int len) {
        for (int i = 0; i < fresh.length; i++) fresh[i] = new String(pool, i * len, len);
    }

    static int hashAll() {
        int s = 0;
        for (String x : fresh) s += x.hashCode();
        return s;
    }
}

#!/bin/sh

staging_dir=$(mktemp -d)
echo "staging dir:" ${staging_dir}

trunk build --release --dist=${staging_dir}

mv -v ${staging_dir}/index.html ${staging_dir}/200.html

surge ${staging_dir} https://rustychat.surge.sh

rm -rv ${staging_dir}

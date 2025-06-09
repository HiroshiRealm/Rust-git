# 首先，清理旧的测试环境（如果存在）
rm -rf /tmp/rust-git-test

# 创建新的测试环境
mkdir /tmp/rust-git-test
cd /tmp/rust-git-test

# 从你的项目目录复制可执行文件过来
cp /home/nihaoran/Rust-git/target/debug/rust-git .

# 在 /tmp/rust-git-test 目录下
mkdir server_repo client_repo bundles

# 进入远程仓库目录
cd server_repo

# 初始化仓库 (使用上一级目录的可执行文件)
../rust-git init

# 创建一个文件并提交
echo "Hello from the server!" > server_file.txt
../rust-git add server_file.txt
../rust-git commit -m "Initial commit on server"

# 返回主测试目录
cd ..



# 1. 从 server_repo "push" 到一个 bundle 文件
cd server_repo
../rust-git push origin ../bundles/initial.bundle
echo "✅ Pushed server content to a bundle file."
cd ..

# 2. 从 client_repo "pull" 这个 bundle 文件
cd client_repo
../rust-git init
../rust-git pull origin ../bundles/initial.bundle
echo "✅ Pulled server bundle into client."

# 3. 验证结果
if [ -f "server_file.txt" ]; then
    echo "✅ SUCCESS: server_file.txt has been pulled into the client."
    cat server_file.txt
else
    echo "❌ FAILURE: server_file.txt was not found in the client."
fi
cd ..




# 1. 在 client_repo 创建并提交一个新文件
cd client_repo
echo "A new file from the client." > client_file.txt
../rust-git add client_file.txt
../rust-git commit -m "Commit from client"

# 2. 从 client "push" 到一个新的 bundle
../rust-git push origin ../bundles/client_update.bundle
echo "✅ Pushed client update to a new bundle file."
cd ..

# 3. 从 server "pull" 客户端的更新
cd server_repo
../rust-git pull origin ../bundles/client_update.bundle
echo "✅ Pulled client bundle into server."

# 4. 验证结果
if [ -f "client_file.txt" ]; then
    echo "✅ SUCCESS: client_file.txt has been pulled into the server."
    cat client_file.txt
else
    echo "❌ FAILURE: client_file.txt was not found in the server."
fi
cd ..
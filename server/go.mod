module github.com/kixelated/warp-sample

go 1.18

require (
	github.com/abema/go-mp4 v0.7.2
	github.com/adriancable/webtransport-go v0.1.0
	github.com/kixelated/invoker v0.9.2
	github.com/lucas-clemente/quic-go v0.27.1
	github.com/zencoder/go-dash/v3 v3.0.2
)

require (
	github.com/cheekybits/genny v1.0.0 // indirect
	github.com/francoispqt/gojay v1.2.13 // indirect
	github.com/fsnotify/fsnotify v1.4.9 // indirect
	github.com/go-task/slim-sprig v0.0.0-20210107165309-348f09dbbbc0 // indirect
	github.com/google/uuid v1.1.2 // indirect
	github.com/marten-seemann/qpack v0.2.1 // indirect
	github.com/marten-seemann/qtls-go1-16 v0.1.5 // indirect
	github.com/marten-seemann/qtls-go1-17 v0.1.1 // indirect
	github.com/marten-seemann/qtls-go1-18 v0.1.1 // indirect
	github.com/nxadm/tail v1.4.8 // indirect
	github.com/onsi/ginkgo v1.16.4 // indirect
	github.com/stretchr/testify v1.7.0 // indirect
	golang.org/x/crypto v0.0.0-20211117183948-ae814b36b871 // indirect
	golang.org/x/mod v0.6.0-dev.0.20220106191415-9b9b3d81d5e3 // indirect
	golang.org/x/net v0.0.0-20211116231205-47ca1ff31462 // indirect
	golang.org/x/sys v0.0.0-20211117180635-dee7805ff2e1 // indirect
	golang.org/x/text v0.3.7 // indirect
	golang.org/x/tools v0.1.11-0.20220316014157-77aa08bb151a // indirect
	golang.org/x/xerrors v0.0.0-20200804184101-5ec99f83aff1 // indirect
	gopkg.in/tomb.v1 v1.0.0-20141024135613-dd632973f1e7 // indirect
)

replace github.com/adriancable/webtransport-go => github.com/kixelated/webtransport-go v0.1.1

replace github.com/lucas-clemente/quic-go => github.com/kixelated/quic-go v0.28.0
